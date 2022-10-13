use anyhow::{Context, Result};
use sqlx::{Pool, Postgres};
use std::ops::Not;
use tokio::io::AsyncRead;

type Sequence = (String, String);

pub struct Relationship {
    pub source_table: String,
    pub dest_table: String,
    pub source_column: String,
    pub dest_column: String,
}

pub struct CopyCmd {
    pub table: String,
    pub clauses: String,
}

impl CopyCmd {
    pub fn new(table: impl Into<String>, clauses: impl Into<String>) -> Self {
        Self {
            table: table.into(),
            clauses: clauses.into(),
        }
    }

    pub fn build_query(&self) -> String {
        format!(
            "COPY (SELECT * FROM {} {}) TO STDOUT CSV HEADER",
            &self.table, &self.clauses
        )
    }
}

pub fn build_queries(
    target_table: &str,
    relationships: &[Relationship],
    sorted_dependencies: &[String],
) -> Vec<CopyCmd> {
    let mut queries = sorted_dependencies
        .iter()
        .map(|dep| {
            let mut iter = relationships.iter().filter(|rel| {
                rel.dest_table == *dep
                    && sorted_dependencies.binary_search(&rel.source_table).is_ok()
            });

            let dependants = iter
                .next()
                .map(|rel| {
                    let mut clause = format!(
                        "WHERE {}.{} IN (SELECT DISTINCT {} FROM {})",
                        rel.dest_table, rel.dest_column, rel.source_column, rel.source_table
                    );

                    clause = iter.fold(clause, |acc, rel| {
                        acc + format!(
                            " OR {}.{} IN (SELECT DISTINCT {} FROM {})",
                            rel.dest_table, rel.dest_column, rel.source_column, rel.source_table
                        )
                        .as_str()
                    });
                    clause
                })
                .unwrap_or_default();

            CopyCmd::new(dep, dependants)
        })
        .collect::<Vec<_>>();

    if queries
        .iter()
        .any(|x| x.table == target_table && x.clauses.is_empty().not())
    {
        queries.push(CopyCmd::new(target_table, ""));
    }

    queries
}

pub struct ImportCmd {
    pub table: String,
    temp_table: String,
    header: String,
}
impl ImportCmd {
    pub fn new<T>(table_name: T, header: impl Into<String>) -> Self
    where
        T: Into<String> + AsRef<str>,
    {
        let temp_table = format!("{}_temp", table_name.as_ref());
        Self {
            table: table_name.into(),
            temp_table,
            header: header.into(),
        }
    }

    pub async fn import<T: AsyncRead + Unpin>(
        &self,
        pool: &Pool<Postgres>,
        reader: T,
    ) -> Result<()> {
        let mut transaction = pool.begin().await?;
        self.create_temp_table(&mut transaction).await?;
        self.copy_csv(&mut transaction, reader).await?;
        self.insert_to(&mut transaction).await?;

        let sequences = self
            .get_sequences(&mut transaction)
            .await
            .with_context(|| format!("unable to get sequences for {}", &self.table))?;

        for seq in sequences {
            self.update_sequence(&mut transaction, &seq)
                .await
                .with_context(|| format!("unable to update sequence {}", seq.0))?;
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn create_temp_table(
        &self,
        transaction: &mut sqlx::Transaction<'_, Postgres>,
    ) -> Result<()> {
        let query = format!(
            "CREATE TEMP TABLE {} (LIKE {} INCLUDING DEFAULTS) ON COMMIT DROP",
            &self.temp_table, &self.table
        );
        sqlx::query(query.as_str())
            .execute(transaction)
            .await
            .with_context(|| format!("unable to create table: {}", &self.temp_table))?;
        Ok(())
    }

    async fn copy_csv<T: AsyncRead + Unpin>(
        &self,
        transaction: &mut sqlx::Transaction<'_, Postgres>,
        reader: T,
    ) -> Result<()> {
        let statement = format!(
            "COPY {}({}) FROM STDIN WITH CSV",
            &self.temp_table, &self.header
        );
        let mut import_cursor = transaction
            .copy_in_raw(&statement)
            .await
            .with_context(|| format!("unable to copy csv data to {}", &self.temp_table))?;

        import_cursor.read_from(reader).await?;
        import_cursor.finish().await?;
        Ok(())
    }

    async fn get_sequences(
        &self,
        transaction: &mut sqlx::Transaction<'_, Postgres>,
    ) -> Result<Vec<Sequence>> {
        let rows = sqlx::query!(
            "
SELECT a.attname AS column_name,
s.relname AS sequence_name
FROM pg_class AS t
JOIN pg_attribute AS a
ON a.attrelid = t.oid
JOIN pg_depend AS d
ON d.refobjid = t.oid
 AND d.refobjsubid = a.attnum
JOIN pg_class AS s
ON s.oid = d.objid
WHERE d.classid = 'pg_catalog.pg_class'::regclass
AND d.refclassid = 'pg_catalog.pg_class'::regclass
AND t.relkind IN ('r', 'P')
AND s.relkind = 'S'
AND t.relname = $1",
            self.table
        )
        .map(|row| (row.sequence_name, row.column_name))
        .fetch_all(transaction)
        .await?;

        Ok(rows)
    }

    async fn update_sequence(
        &self,
        transaction: &mut sqlx::Transaction<'_, Postgres>,
        sequence: &Sequence,
    ) -> Result<()> {
        sqlx::query("SELECT setval(?, ?) FROM ?")
            .bind(&sequence.0)
            .bind(&sequence.1)
            .bind(&self.table)
            .execute(transaction)
            .await?;
        Ok(())
    }

    async fn insert_to(&self, transaction: &mut sqlx::Transaction<'_, Postgres>) -> Result<()> {
        let query = format!(
            "INSERT INTO {}({}) SELECT {} FROM {} ON CONFLICT DO NOTHING",
            &self.table, &self.header, &self.header, &self.temp_table
        );
        sqlx::query(query.as_str())
            .execute(transaction)
            .await
            .with_context(|| {
                format!(
                    "unable to insert data from {} to {}",
                    &self.temp_table, &self.table
                )
            })?;

        Ok(())
    }
}

pub async fn get_relationships(pool: &Pool<Postgres>) -> Result<Vec<Relationship>> {
    let relationships = sqlx::query!(
        "
SELECT
x.table_name as source_table,
x.column_name as source_column,
y.table_name as dest_table,
y.column_name as dest_column
FROM information_schema.referential_constraints c
JOIN information_schema.key_column_usage x on x.constraint_name = c.constraint_name
JOIN information_schema.key_column_usage y on y.ordinal_position = x.position_in_unique_constraint
AND y.constraint_name = c.unique_constraint_name"
    )
    .fetch_all(pool)
    .await
    .with_context(|| "unable to fetch list of foreign keys")?;

    Ok(relationships
        .into_iter()
        .filter_map(|rel| {
            let source_table = rel.source_table?;
            let dest_table = rel.dest_table?;
            let source_column = rel.source_column?;
            let dest_column = rel.dest_column?;
            Some(Relationship {
                source_table,
                dest_table,
                source_column,
                dest_column,
            })
        })
        .collect())
}

pub async fn get_all_tables(pool: &Pool<Postgres>) -> Result<Vec<String>> {
    let tables = sqlx::query!(
        "
SELECT cls.relname as name
FROM pg_class cls
JOIN pg_namespace nsp ON nsp.oid = cls.relnamespace
WHERE nsp.nspname NOT IN ('information_schema', 'pg_catalog')
AND cls.relkind = 'r'"
    )
    .fetch_all(pool)
    .await
    .with_context(|| "unable to fetch_all list of all tables")?;

    Ok(tables.into_iter().filter_map(|t| t.name).collect())
}
