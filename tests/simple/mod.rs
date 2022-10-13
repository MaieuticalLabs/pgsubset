extern crate pgsubset;
use std::collections::HashMap;

use pgsubset::config::{Config, TransformKind};
use pgsubset::run;
use sqlx::postgres::PgRow;
use sqlx::Row;

#[sqlx_database_tester::test(pool(variable = "pool", migrations = "./tests/simple/migrations"))]
async fn test_export() {
    let target_dir = "./tests/simple/export_csv";
    sqlx::query(
        "INSERT INTO table_1(id, name) VALUES (1, 'entry_1'), (2, 'entry_2'), (3, 'entry_3')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO table_2(id, table_1_id, name) VALUES (1, 1, 'entry_1'), (2, 2,'entry_2'), (3, 3, 'entry_3')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO table_3(id, table_2_id, name) VALUES (1, 1, 'entry_1'), (2, 2,'entry_2'), (3, 3, 'entry_3')").execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO table_4(id, name) VALUES (1, 'entry_1'), (2, 'entry_2'), (3, 'entry_3')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let cfg = Config {
        target_table: "table_3".to_string(),
        target_dir: target_dir.to_string(),
        database_url: "".to_string(),
        m2m_tables: None,
        transforms: None,
    };

    run::export(&pool, cfg).await.unwrap();

    let table_1 = tokio::fs::read_to_string(format!("{}/00-table_1.csv", target_dir))
        .await
        .unwrap();
    let table_2 = tokio::fs::read_to_string(format!("{}/01-table_2.csv", target_dir))
        .await
        .unwrap();
    let table_3 = tokio::fs::read_to_string(format!("{}/02-table_3.csv", target_dir))
        .await
        .unwrap();
    let table_4 = tokio::fs::metadata(format!("{}/03-table_4.csv", target_dir)).await;
    assert!(table_4.is_err());

    teardown(target_dir).await;
    assert_eq!(table_1, "id,name\n1,entry_1\n2,entry_2\n3,entry_3\n");
    assert_eq!(
        table_2,
        "id,table_1_id,name\n1,1,entry_1\n2,2,entry_2\n3,3,entry_3\n"
    );
    assert_eq!(
        table_3,
        "id,table_2_id,name\n1,1,entry_1\n2,2,entry_2\n3,3,entry_3\n"
    );
}

#[sqlx_database_tester::test(pool(variable = "pool", migrations = "./tests/simple/migrations"))]
async fn test_import() {
    let target_dir = "./tests/simple/csv";
    let cfg = Config {
        target_table: "table_3".to_string(),
        target_dir: target_dir.to_string(),
        database_url: "".to_string(),
        m2m_tables: None,
        transforms: None,
    };

    run::import(&pool, cfg).await.unwrap();
    let table_1 = sqlx::query("SELECT id, name FROM table_1")
        .fetch_all(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row: PgRow| {
            format!(
                "{},{}",
                row.try_get::<i32, &str>("id").unwrap(),
                row.try_get::<&str, &str>("name").unwrap()
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    let table_2 = sqlx::query("SELECT id, table_1_id, name FROM table_2")
        .fetch_all(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row: PgRow| {
            format!(
                "{},{},{}",
                row.try_get::<i32, &str>("id").unwrap(),
                row.try_get::<i32, &str>("table_1_id").unwrap(),
                row.try_get::<&str, &str>("name").unwrap()
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    let table_3 = sqlx::query("SELECT id, table_2_id, name FROM table_3")
        .fetch_all(&pool)
        .await
        .unwrap()
        .into_iter()
        .map(|row: PgRow| {
            format!(
                "{},{},{}",
                row.try_get::<i32, &str>("id").unwrap(),
                row.try_get::<i32, &str>("table_2_id").unwrap(),
                row.try_get::<&str, &str>("name").unwrap()
            )
        })
        .collect::<Vec<String>>()
        .join("\n");

    assert_eq!(table_1, "1,entry_1\n2,entry_2\n3,entry_3");
    assert_eq!(table_2, "1,1,entry_1\n2,2,entry_2\n3,3,entry_3");
    assert_eq!(table_3, "1,1,entry_1\n2,2,entry_2\n3,3,entry_3");
}

#[sqlx_database_tester::test(pool(variable = "pool", migrations = "./tests/simple/migrations"))]
async fn test_transforms() {
    let target_dir = "./tests/simple/transforms_csv";
    sqlx::query(
        "INSERT INTO table_1(id, name) VALUES (1, 'entry_1'), (2, 'entry_2'), (3, 'entry_3')",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO table_2(id, table_1_id, name) VALUES (1, 1, 'entry_1'), (2, 2,'entry_2'), (3, 3, 'entry_3')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO table_3(id, table_2_id, name) VALUES (1, 1, 'entry_1'), (2, 2,'entry_2'), (3, 3, 'entry_3')").execute(&pool).await.unwrap();
    sqlx::query(
        "INSERT INTO table_4(id, name) VALUES (1, 'entry_1'), (2, 'entry_2'), (3, 'entry_3')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let mut name_transform = HashMap::new();
    name_transform.insert("id".to_string(), TransformKind::ClearField);

    let mut transforms = HashMap::new();
    transforms.insert("table_1".to_string(), name_transform);
    let cfg = Config {
        target_table: "table_3".to_string(),
        target_dir: target_dir.to_string(),
        database_url: "".to_string(),
        m2m_tables: None,
        transforms: Some(transforms),
    };

    run::export(&pool, cfg).await.unwrap();

    let table_1 = tokio::fs::read_to_string(format!("{}/00-table_1.csv", target_dir))
        .await
        .unwrap();

    teardown(target_dir).await;
    assert_eq!(table_1, "id,name\n,entry_1\n,entry_2\n,entry_3\n");
}

async fn teardown(dir: &str) {
    tokio::fs::remove_dir_all(dir).await.unwrap()
}
