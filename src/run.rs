use crate::config::Config;
use crate::graph::{relationships_as_edges, tables_as_nodes, DepGraph};
use crate::sql::{build_queries, get_all_tables, get_relationships, ImportCmd};
use crate::transform::{IndexedTransforms, TableTransform, Transform, Transforms};
use anyhow::{anyhow, Context, Result};
use futures::future::{join_all, OptionFuture};
use futures::stream::TryStreamExt;
use regex::Regex;
use sqlx::{Pool, Postgres};
use std::borrow::Cow;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub async fn export(pool: &Pool<Postgres>, cfg: Config) -> Result<()> {
    tokio::fs::create_dir_all(&cfg.target_dir).await?;

    let relationships = get_relationships(pool).await?;
    let tables = get_all_tables(pool).await?;
    let m2m_tables = Arc::new(cfg.m2m_tables.unwrap_or_default());

    let transforms = cfg
        .transforms
        .map(|transforms| {
            transforms
                .into_iter()
                .map(|(key, val)| {
                    let table_transforms = val
                        .into_iter()
                        .map(|(k, v)| (k, Transform::new(&v)))
                        .collect::<TableTransform>();

                    (key, table_transforms)
                })
                .collect::<Transforms>()
        })
        .unwrap_or_default();
    let transforms = Arc::new(transforms);

    let nodes = tables_as_nodes(&tables);
    let edges = relationships_as_edges(&relationships, &m2m_tables);
    let graph = DepGraph::new(nodes, edges)?;
    let sorted_dependencies = graph.get_dependencies_of(cfg.target_table.as_str())?;

    let target_path = Arc::new(PathBuf::from(&cfg.target_dir));

    let queries = build_queries(&cfg.target_table, &relationships, &sorted_dependencies);
    let cmd_len = queries.len();
    let mut handles = Vec::with_capacity(cmd_len);

    for (idx, q) in queries.into_iter().enumerate() {
        let mut conn = pool
            .clone()
            .acquire()
            .await
            .with_context(|| "unable to acquire connection to database")?;

        let path = Arc::clone(&target_path);
        let m2m = Arc::clone(&m2m_tables);
        let trans = Arc::clone(&transforms);

        handles.push(tokio::task::spawn(async move {
            let table_name = q.table.as_str();
            let ts = trans.get(table_name);

            let mut csv_path_index = idx;
            if m2m.iter().any(|x| x.name == table_name) {
                csv_path_index += cmd_len;
            }
            let csv_path = format!("{csv_path_index:02}-{table_name}.csv");
            let full_path = path.join(csv_path.as_str());

            let mut data = conn
                .copy_out_raw(q.build_query().as_str())
                .await
                .with_context(|| format!("unable to perform copy operation from {}", &q.table))?;
            let mut file = File::create(&full_path)
                .await
                .with_context(|| format!("unable to create file {}", full_path.display()))?;

            let ts_with_idx: OptionFuture<_> = ts
                .map(|t| async {
                    let c = data
                        .try_next()
                        .await?
                        .ok_or_else(|| anyhow!("error fetching headers for {}", table_name))?;

                    file.write_all(&c).await?;
                    let s = std::str::from_utf8(&c)
                        .map_err(|err| anyhow!("error decoding headers for {table_name}: {err}"))?;

                    let header: Vec<_> = s.split(',').map(|s| s.to_owned()).collect();
                    Ok::<_, anyhow::Error>(IndexedTransforms::new(t, &header))
                })
                .into();

            let ts_with_idx = ts_with_idx.await.transpose()?.unwrap_or_default();

            while let Some(chunk) = data.try_next().await? {
                if !ts_with_idx.transforms.is_empty() {
                    let components = std::str::from_utf8(&chunk)
                        .map_err(|err| anyhow!("error decoding row for {table_name}: {err}"))?;

                    let mut iter = components.split(',').enumerate().map(|(index, component)| {
                        ts_with_idx
                            .transforms
                            .get(&index)
                            .map(|f| Cow::Owned((f.func)(component)))
                            .unwrap_or(Cow::Borrowed(component))
                    });

                    if let Some(s) = iter.next() {
                        file.write_all(s.as_bytes()).await?;

                        for s in iter {
                            file.write_all(b",").await?;
                            file.write_all(s.as_bytes()).await?;
                        }
                    }
                } else {
                    file.write_all(&chunk).await?;
                }
            }
            println!("{} writed", full_path.display());
            Ok::<_, anyhow::Error>(())
        }));
    }

    join_all(handles)
        .await
        .into_iter()
        .flatten()
        .collect::<Result<Vec<()>, _>>()
        .with_context(|| "something went wrong fetching task handles")?;
    Ok(())
}

pub async fn import(pool: &Pool<Postgres>, cfg: Config) -> Result<()> {
    let target_path = PathBuf::from(&cfg.target_dir);
    let mut dir_entries = fs::read_dir(target_path)
        .await
        .with_context(|| format!("error opening target_dir: {}", &cfg.target_dir))?;

    let mut csvs: Vec<PathBuf> = Vec::new();
    while let Some(entry) = dir_entries.next_entry().await? {
        let path = entry.path();
        if let Some(v) = path.extension() {
            if v == "csv" {
                csvs.push(path);
            }
        }
    }
    csvs.sort();

    let re = Regex::new(r"\d\d-(.*)\.csv$")?;

    let mut header = String::new();

    for csv in csvs {
        let fd = File::open(&csv)
            .await
            .with_context(|| format!("error opening {}", csv.display()))?;
        let mut reader = BufReader::new(fd);
        header.clear();
        reader.read_line(&mut header).await?;

        let csv_name = match csv.file_name() {
            Some(name) => name
                .to_str()
                .with_context(|| format!("Invalid filename {}", csv.display()))?,
            None => continue,
        };
        let table = re
            .captures(csv_name)
            .with_context(|| format!("Unable to parse the csv filename {}", csv.display()))?;

        let import_cmd = ImportCmd::new(&table[1], header.as_str());
        import_cmd.import(pool, reader).await?;
        println!("imported {} to {}", &csv.display(), import_cmd.table);
    }
    Ok(())
}
