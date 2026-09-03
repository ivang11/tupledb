use app_lib::driver::{DatabaseDriver, KeysetPage, RowChange, RowDeletion, TableChange};
use app_lib::filters::{FilterRow, FilterSet, Operator};
use app_lib::mysql::{export_table_file, MySqlDriver};
use app_lib::schema::{export_database_file, import_sql_file, ExportOptions};
use serde_json::{json, Value};
use sqlx::{MySqlPool, Row};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

async fn test_pool() -> MySqlPool {
    let url = std::env::var("TUPLEDB_TEST_MYSQL_URL").expect(
        "Set TUPLEDB_TEST_MYSQL_URL, for example mysql://root:password@127.0.0.1:3306/mysql",
    );
    MySqlPool::connect(&url)
        .await
        .expect("connect to TUPLEDB_TEST_MYSQL_URL")
}

async fn setup_database(pool: &MySqlPool) -> String {
    let db = format!("tupledb_it_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("CREATE DATABASE `{}`", db))
        .execute(pool)
        .await
        .expect("create test database");
    db
}

async fn drop_database(pool: &MySqlPool, db: &str) {
    let _ = sqlx::query(&format!("DROP DATABASE IF EXISTS `{}`", db))
        .execute(pool)
        .await;
}

async fn seed_people(pool: &MySqlPool, db: &str) {
    sqlx::query(&format!(
        "CREATE TABLE `{}`.`people` (
            id INT PRIMARY KEY,
            name VARCHAR(64) NOT NULL,
            empty_text VARCHAR(64) NOT NULL,
            nullable_text VARCHAR(64) NULL,
            price DECIMAL(10,2) NOT NULL,
            active TINYINT(1) NOT NULL,
            meta JSON NULL
        )",
        db
    ))
    .execute(pool)
    .await
    .expect("create people table");

    sqlx::query(&format!(
        "INSERT INTO `{}`.`people`
            (id, name, empty_text, nullable_text, price, active, meta)
         VALUES
            (1, 'Ada', '', NULL, 12.30, 1, JSON_OBJECT('role', 'admin')),
            (2, 'Grace', '', 'hello', 7.00, 0, NULL),
            (3, 'Linus', '', 'kernel', 9.99, 1, JSON_OBJECT('role', 'dev'))",
        db
    ))
    .execute(pool)
    .await
    .expect("insert people rows");
}

fn object_at(rows: &[Value], index: usize) -> &serde_json::Map<String, Value> {
    rows[index].as_object().expect("row should be object")
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn get_table_data_preserves_null_empty_and_common_types() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = MySqlDriver::new(pool.clone(), false);

    seed_people(&pool, &db).await;

    let result = driver
        .get_table_data(
            &db,
            "people",
            0,
            10,
            None,
            Some("id".into()),
            Some(false),
            true,
            None,
        )
        .await
        .expect("fetch table data");

    assert_eq!(result.total_count, 3);
    assert!(!result.total_count_is_estimate);
    assert_eq!(result.rows.len(), 3);

    let first = object_at(&result.rows, 0);
    assert_eq!(first["id"], json!(1));
    assert_eq!(first["name"], json!("Ada"));
    assert_eq!(first["empty_text"], json!(""));
    assert_eq!(first["nullable_text"], Value::Null);
    assert_eq!(first["price"], json!("12.30"));
    assert_eq!(first["active"], json!(true));
    assert_eq!(first["meta"], json!("{\"role\": \"admin\"}"));

    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn filters_and_sort_are_applied_by_driver() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = MySqlDriver::new(pool.clone(), false);

    seed_people(&pool, &db).await;

    let filters = FilterSet {
        match_all: true,
        rows: vec![FilterRow {
            active: true,
            column: "active".into(),
            operator: Operator::Equals,
            value: "true".into(),
        }],
    };

    let result = driver
        .get_table_data(
            &db,
            "people",
            0,
            10,
            Some(filters),
            Some("name".into()),
            Some(true),
            true,
            None,
        )
        .await
        .expect("fetch filtered data");

    let names: Vec<_> = result
        .rows
        .iter()
        .map(|row| row["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["Linus".to_string(), "Ada".to_string()]);

    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn keyset_pagination_fetches_next_page_after_cursor() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = MySqlDriver::new(pool.clone(), false);

    seed_people(&pool, &db).await;

    let first_page = driver
        .get_table_data(
            &db,
            "people",
            0,
            2,
            None,
            Some("id".into()),
            Some(false),
            true,
            None,
        )
        .await
        .expect("fetch first page");
    let last_id = object_at(&first_page.rows, 1)["id"].clone();

    let second_page = driver
        .get_table_data(
            &db,
            "people",
            1,
            2,
            None,
            None,
            None,
            true,
            Some(KeysetPage {
                column: "id".into(),
                value: last_id,
                direction: "next".into(),
            }),
        )
        .await
        .expect("fetch keyset page");

    assert_eq!(second_page.rows.len(), 1);
    assert_eq!(object_at(&second_page.rows, 0)["id"], json!(3));

    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn views_are_listed_as_views_and_readable() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = MySqlDriver::new(pool.clone(), false);

    seed_people(&pool, &db).await;
    sqlx::query(&format!(
        "CREATE VIEW `{}`.`active_people` AS SELECT id, name FROM `{}`.`people` WHERE active = 1",
        db, db
    ))
    .execute(&pool)
    .await
    .expect("create view");

    let tables = driver.get_tables(&db).await.expect("list tables");
    let view = tables
        .iter()
        .find(|table| table.name == "active_people")
        .expect("view should be listed");
    assert!(view.table_type.to_uppercase().contains("VIEW"));

    let result = driver
        .get_table_data(
            &db,
            "active_people",
            0,
            10,
            None,
            Some("id".into()),
            Some(false),
            true,
            None,
        )
        .await
        .expect("read view data");
    assert_eq!(result.total_count, 2);
    assert_eq!(object_at(&result.rows, 0)["name"], json!("Ada"));

    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn get_table_structure_marks_primary_key_and_nullable_columns() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = MySqlDriver::new(pool.clone(), false);

    seed_people(&pool, &db).await;

    let structure = driver
        .get_table_structure(&db, "people")
        .await
        .expect("fetch table structure");
    let id = structure.iter().find(|col| col.field == "id").unwrap();
    let nullable = structure
        .iter()
        .find(|col| col.field == "nullable_text")
        .unwrap();

    assert_eq!(id.key, "PRI");
    assert!(!id.nullable);
    assert!(nullable.nullable);

    let count: i64 = sqlx::query("SELECT COUNT(*) FROM information_schema.tables")
        .fetch_one(&pool)
        .await
        .expect("server remains usable")
        .try_get(0)
        .expect("count");
    assert!(count > 0);

    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn import_session_executes_representative_dump_in_batches() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = MySqlDriver::new(pool.clone(), false);
    let import_id = format!("import_{}", Uuid::new_v4().simple());

    driver
        .begin_import_session(&db, &import_id)
        .await
        .expect("begin import session");

    let first_batch = vec![
        "CREATE TABLE imported (id INT PRIMARY KEY, note VARCHAR(255), flag TINYINT(1), meta JSON NULL)".to_string(),
        "INSERT INTO imported (id, note, flag, meta) VALUES (1, 'semi;colon', 1, JSON_OBJECT('kind', 'first'))".to_string(),
        "INSERT INTO imported (id, note, flag, meta) VALUES (2, 'literal -- not comment', 0, NULL)".to_string(),
    ];
    let second_batch = vec![
        "/* block comment */ INSERT INTO imported (id, note, flag, meta) VALUES (3, 'literal /* not comment */', 1, JSON_OBJECT('kind', 'third'))".to_string(),
        "INSERT INTO imported (id, note, flag, meta) VALUES (4, 'batch two', 1, NULL)".to_string(),
    ];

    let first_results = driver
        .execute_statements(&db, &first_batch, Some(&import_id))
        .await;
    assert!(first_results.iter().all(Result::is_ok));

    let second_results = driver
        .execute_statements(&db, &second_batch, Some(&import_id))
        .await;
    assert!(second_results.iter().all(Result::is_ok));

    driver
        .finish_import_session(&import_id)
        .await
        .expect("finish import session");

    let result = driver
        .get_table_data(
            &db,
            "imported",
            0,
            10,
            None,
            Some("id".into()),
            Some(false),
            true,
            None,
        )
        .await
        .expect("read imported rows");

    assert_eq!(result.total_count, 4);
    assert_eq!(object_at(&result.rows, 0)["note"], json!("semi;colon"));
    assert_eq!(
        object_at(&result.rows, 1)["note"],
        json!("literal -- not comment")
    );
    assert_eq!(
        object_at(&result.rows, 2)["note"],
        json!("literal /* not comment */")
    );

    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn import_sql_file_runs_dump_and_reports_metrics() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver: Arc<dyn DatabaseDriver> = Arc::new(MySqlDriver::new(pool.clone(), false));
    let import_id = format!("import_file_{}", Uuid::new_v4().simple());
    let path = std::env::temp_dir().join(format!("{}.sql", import_id));
    let large_note = "x".repeat(700_000);
    let dump = format!(
        "-- leading comment
        CREATE TABLE imported_file (
            id INT PRIMARY KEY,
            note LONGTEXT NULL,
            flag TINYINT(1) NOT NULL
        );
        INSERT INTO imported_file (id, note, flag) VALUES (1, 'semi;colon', 1);
        INSERT INTO imported_file (id, note, flag) VALUES (2, 'literal -- not comment', 0);
        /* block comment */
        INSERT INTO imported_file (id, note, flag) VALUES (3, 'literal /* not comment */', 1);
        INSERT INTO imported_file (id, note, flag) VALUES (4, '{}', 1);
        INSERT INTO imported_file (id, note, flag) VALUES (5, '{}', 0);
        ",
        large_note, large_note
    );
    std::fs::write(&path, dump).expect("write import dump");

    let progress_statuses = Arc::new(Mutex::new(Vec::new()));
    let progress_for_cb = Arc::clone(&progress_statuses);
    let result = import_sql_file(
        driver.clone(),
        &db,
        path.to_str().expect("temp path should be utf-8"),
        &import_id,
        &|| false,
        &move |progress| {
            progress_for_cb.lock().unwrap().push(progress.status);
        },
    )
    .await
    .expect("import sql file");

    assert_eq!(result.executed, 6);
    assert!(result.errors.is_empty());
    assert_eq!(result.metrics.parsed_statements, 6);
    assert!(result.metrics.compacted_statements >= 2);
    assert!(result.metrics.executed_batches >= 2);
    assert!(result.metrics.sql_blocks >= 2);
    assert!(progress_statuses
        .lock()
        .unwrap()
        .iter()
        .any(|status| status.contains("Import complete")));

    let rows = driver
        .get_table_data(
            &db,
            "imported_file",
            0,
            10,
            None,
            Some("id".into()),
            Some(false),
            true,
            None,
        )
        .await
        .expect("read imported file rows");
    assert_eq!(rows.total_count, 5);
    assert_eq!(object_at(&rows.rows, 0)["note"], json!("semi;colon"));
    assert_eq!(
        object_at(&rows.rows, 1)["note"],
        json!("literal -- not comment")
    );
    assert_eq!(
        object_at(&rows.rows, 2)["note"],
        json!("literal /* not comment */")
    );

    let _ = std::fs::remove_file(path);
    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn export_table_file_writes_csv_json_sql_and_sql_can_be_reimported() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let reimport_db = setup_database(&pool).await;
    let driver: Arc<dyn DatabaseDriver> = Arc::new(MySqlDriver::new(pool.clone(), false));
    let export_id = format!("export_file_{}", Uuid::new_v4().simple());
    let csv_path = std::env::temp_dir().join(format!("{}.csv", export_id));
    let json_path = std::env::temp_dir().join(format!("{}.json", export_id));
    let sql_path = std::env::temp_dir().join(format!("{}.sql", export_id));

    let create_table = "CREATE TABLE exported_table (
        id INT PRIMARY KEY,
        name VARCHAR(64) NULL,
        note TEXT NULL,
        active TINYINT(1) NOT NULL
    )";
    sqlx::query(&format!(
        "CREATE TABLE `{}`.`exported_table` {}",
        db,
        &create_table["CREATE TABLE exported_table ".len()..]
    ))
    .execute(&pool)
    .await
    .expect("create export table");
    sqlx::query(&format!(
        "INSERT INTO `{}`.`exported_table` (id, name, note, active) VALUES
            (1, 'Ada', 'comma, quote \" and newline\ninside', 1),
            (2, NULL, '', 0),
            (3, 'O\\'Reilly', 'semi;colon', 1)",
        db
    ))
    .execute(&pool)
    .await
    .expect("insert export rows");

    let progress_statuses = Arc::new(Mutex::new(Vec::new()));
    for (format, path) in [
        ("csv", csv_path.clone()),
        ("json", json_path.clone()),
        ("sql", sql_path.clone()),
    ] {
        let progress_for_cb = Arc::clone(&progress_statuses);
        let rows = export_table_file(
            driver.clone(),
            db.clone(),
            "exported_table".to_string(),
            format.to_string(),
            path.to_str()
                .expect("temp path should be utf-8")
                .to_string(),
            &move |_current, _total, status| {
                progress_for_cb.lock().unwrap().push(status);
            },
        )
        .await
        .unwrap_or_else(|e| panic!("export {format}: {e}"));
        assert_eq!(rows, 3);
    }

    let csv = std::fs::read_to_string(&csv_path).expect("read csv export");
    assert!(csv.starts_with("id,name,note,active\n"));
    assert!(csv.contains("\"comma, quote \"\" and newline\ninside\""));
    assert!(csv.contains("2,,"));

    let json_rows: Value =
        serde_json::from_str(&std::fs::read_to_string(&json_path).expect("read json export"))
            .expect("parse json export");
    let json_rows = json_rows
        .as_array()
        .expect("json export should be an array");
    assert_eq!(json_rows.len(), 3);
    assert_eq!(json_rows[0]["active"], json!(true));
    assert_eq!(json_rows[1]["name"], Value::Null);

    sqlx::query(&format!(
        "CREATE TABLE `{}`.`exported_table` {}",
        reimport_db,
        &create_table["CREATE TABLE exported_table ".len()..]
    ))
    .execute(&pool)
    .await
    .expect("create reimport table");

    let import_id = format!("reimport_{}", Uuid::new_v4().simple());
    let import_result = import_sql_file(
        driver.clone(),
        &reimport_db,
        sql_path.to_str().expect("temp path should be utf-8"),
        &import_id,
        &|| false,
        &|_progress| {},
    )
    .await
    .expect("reimport sql export");
    assert_eq!(import_result.executed, 3);
    assert!(import_result.errors.is_empty());

    let reimported = driver
        .get_table_data(
            &reimport_db,
            "exported_table",
            0,
            10,
            None,
            Some("id".into()),
            Some(false),
            true,
            None,
        )
        .await
        .expect("read reimported rows");
    assert_eq!(reimported.total_count, 3);
    assert_eq!(
        object_at(&reimported.rows, 0)["note"],
        json!("comma, quote \" and newline\ninside")
    );
    assert_eq!(object_at(&reimported.rows, 1)["name"], Value::Null);
    assert_eq!(object_at(&reimported.rows, 2)["name"], json!("O'Reilly"));
    assert!(progress_statuses
        .lock()
        .unwrap()
        .iter()
        .any(|status| status == "Export complete"));

    let _ = std::fs::remove_file(csv_path);
    let _ = std::fs::remove_file(json_path);
    let _ = std::fs::remove_file(sql_path);
    drop_database(&pool, &db).await;
    drop_database(&pool, &reimport_db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn export_database_file_writes_full_dump_and_sql_can_be_reimported() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let reimport_db = setup_database(&pool).await;
    let driver: Arc<dyn DatabaseDriver> = Arc::new(MySqlDriver::new(pool.clone(), false));
    let export_id = format!("export_db_{}", Uuid::new_v4().simple());
    let sql_path = std::env::temp_dir().join(format!("{}.sql", export_id));

    sqlx::query(&format!(
        "CREATE TABLE `{}`.`authors` (
            id INT PRIMARY KEY,
            name VARCHAR(64) NOT NULL
        )",
        db
    ))
    .execute(&pool)
    .await
    .expect("create authors");
    sqlx::query(&format!(
        "CREATE TABLE `{}`.`books` (
            id INT PRIMARY KEY,
            author_id INT NOT NULL,
            title VARCHAR(128) NOT NULL,
            note TEXT NULL,
            CONSTRAINT fk_books_authors FOREIGN KEY (author_id) REFERENCES authors(id)
        )",
        db
    ))
    .execute(&pool)
    .await
    .expect("create books");
    sqlx::query(&format!(
        "INSERT INTO `{}`.`authors` (id, name) VALUES
            (1, 'Ada'),
            (2, 'Grace');
         INSERT INTO `{}`.`books` (id, author_id, title, note) VALUES
            (1, 1, 'Analytical Engine', 'quote \\' and comma, ok'),
            (2, 2, 'Compiler', NULL)",
        db, db
    ))
    .execute(&pool)
    .await
    .expect("insert export database rows");

    let progress_statuses = Arc::new(Mutex::new(Vec::new()));
    let progress_for_cb = Arc::clone(&progress_statuses);
    let exported_rows = export_database_file(
        driver.clone(),
        db.clone(),
        "full".to_string(),
        sql_path
            .to_str()
            .expect("temp path should be utf-8")
            .to_string(),
        None,
        "sql",
        ExportOptions::default(),
        &move |progress| {
            progress_for_cb.lock().unwrap().push(progress.status);
        },
        &|| false,
    )
    .await
    .expect("export database file");
    assert_eq!(exported_rows, 4);

    let dump = std::fs::read_to_string(&sql_path).expect("read database export");
    assert!(dump.contains("-- Database:"));
    assert!(dump.contains("SET FOREIGN_KEY_CHECKS=0;"));
    assert!(dump.contains("CREATE TABLE"));
    assert!(dump.contains("INSERT INTO `authors`"));
    assert!(dump.contains("INSERT INTO `books`"));
    assert!(dump.contains("SET FOREIGN_KEY_CHECKS=1;"));

    let import_id = format!("reimport_db_{}", Uuid::new_v4().simple());
    let import_result = import_sql_file(
        driver.clone(),
        &reimport_db,
        sql_path.to_str().expect("temp path should be utf-8"),
        &import_id,
        &|| false,
        &|_progress| {},
    )
    .await
    .expect("reimport database export");
    assert!(import_result.errors.is_empty());
    assert!(import_result.executed >= 8);

    let authors = driver
        .get_table_data(
            &reimport_db,
            "authors",
            0,
            10,
            None,
            Some("id".into()),
            Some(false),
            true,
            None,
        )
        .await
        .expect("read reimported authors");
    let books = driver
        .get_table_data(
            &reimport_db,
            "books",
            0,
            10,
            None,
            Some("id".into()),
            Some(false),
            true,
            None,
        )
        .await
        .expect("read reimported books");
    assert_eq!(authors.total_count, 2);
    assert_eq!(books.total_count, 2);
    assert_eq!(
        object_at(&books.rows, 0)["note"],
        json!("quote ' and comma, ok")
    );
    assert_eq!(object_at(&books.rows, 1)["note"], Value::Null);
    assert!(progress_statuses
        .lock()
        .unwrap()
        .iter()
        .any(|status| status == "Export complete"));

    let _ = std::fs::remove_file(sql_path);
    drop_database(&pool, &db).await;
    drop_database(&pool, &reimport_db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn destructive_operations_handle_fk_checks_and_unusual_names() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = MySqlDriver::new(pool.clone(), false);
    let dropped_db = format!("tupledb_drop_{}", Uuid::new_v4().simple());

    sqlx::query(&format!(
        "CREATE TABLE `{}`.`parent table` (id INT PRIMARY KEY)",
        db
    ))
    .execute(&pool)
    .await
    .expect("create parent table");
    sqlx::query(&format!(
        "CREATE TABLE `{}`.`child-table` (
            id INT PRIMARY KEY AUTO_INCREMENT,
            parent_id INT NOT NULL,
            CONSTRAINT fk_child_parent FOREIGN KEY (parent_id) REFERENCES `parent table`(id)
        )",
        db
    ))
    .execute(&pool)
    .await
    .expect("create child table");
    sqlx::query(&format!(
        "INSERT INTO `{}`.`parent table` (id) VALUES (1);
         INSERT INTO `{}`.`child-table` (parent_id) VALUES (1)",
        db, db
    ))
    .execute(&pool)
    .await
    .expect("insert fk rows");

    driver
        .truncate_table(&db, "child-table", true)
        .await
        .expect("truncate child with fk checks disabled");
    let remaining: i64 = sqlx::query(&format!("SELECT COUNT(*) FROM `{}`.`child-table`", db))
        .fetch_one(&pool)
        .await
        .expect("count child rows")
        .try_get(0)
        .expect("child count");
    assert_eq!(remaining, 0);

    driver
        .drop_table(&db, "parent table", true)
        .await
        .expect("drop parent with unusual name");
    driver
        .drop_tables(&db, &["child-table".to_string()], true)
        .await
        .expect("drop child in bulk");

    driver
        .create_database(&dropped_db, None, None)
        .await
        .expect("create database through driver");
    assert!(driver
        .get_databases()
        .await
        .expect("list databases")
        .contains(&dropped_db));
    driver
        .drop_database(&dropped_db)
        .await
        .expect("drop database through driver");
    assert!(!driver
        .get_databases()
        .await
        .expect("list databases after drop")
        .contains(&dropped_db));

    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn insert_row_and_apply_table_changes_preserve_edit_semantics() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = MySqlDriver::new(pool.clone(), false);

    sqlx::query(&format!(
        "CREATE TABLE `{}`.`edits` (
            id INT PRIMARY KEY,
            note VARCHAR(64) NULL,
            active TINYINT(1) NOT NULL,
            created_at DATETIME NULL
        )",
        db
    ))
    .execute(&pool)
    .await
    .expect("create edits table");

    driver
        .insert_row(
            &db,
            "edits",
            vec![
                TableChange {
                    column: "id".into(),
                    value: json!(1),
                },
                TableChange {
                    column: "note".into(),
                    value: json!(""),
                },
                TableChange {
                    column: "active".into(),
                    value: json!(true),
                },
                TableChange {
                    column: "created_at".into(),
                    value: json!("NOW()"),
                },
            ],
            false,
        )
        .await
        .expect("insert row with expression");
    driver
        .insert_row(
            &db,
            "edits",
            vec![
                TableChange {
                    column: "id".into(),
                    value: json!(2),
                },
                TableChange {
                    column: "note".into(),
                    value: Value::Null,
                },
                TableChange {
                    column: "active".into(),
                    value: json!(false),
                },
            ],
            false,
        )
        .await
        .expect("insert row with null and boolean");

    driver
        .apply_table_changes(
            &db,
            "edits",
            vec![RowChange {
                pk_column: "id".into(),
                pk_value: json!(1),
                changes: vec![
                    TableChange {
                        column: "note".into(),
                        value: Value::Null,
                    },
                    TableChange {
                        column: "active".into(),
                        value: json!(false),
                    },
                ],
            }],
            vec![RowDeletion {
                pk_column: "id".into(),
                pk_value: json!(2),
            }],
            false,
        )
        .await
        .expect("apply update and delete");

    let rows = driver
        .get_table_data(
            &db,
            "edits",
            0,
            10,
            None,
            Some("id".into()),
            Some(false),
            true,
            None,
        )
        .await
        .expect("read edited rows");

    assert_eq!(rows.total_count, 1);
    let row = object_at(&rows.rows, 0);
    assert_eq!(row["id"], json!(1));
    assert_eq!(row["note"], Value::Null);
    assert_eq!(row["active"], json!(false));
    assert!(row["created_at"].as_str().is_some_and(|s| !s.is_empty()));

    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn rare_mysql_types_are_parsed_to_stable_json_values() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = MySqlDriver::new(pool.clone(), false);

    sqlx::query(&format!(
        "CREATE TABLE `{}`.`rare_types` (
            id INT PRIMARY KEY,
            bit_value BIT(4) NOT NULL,
            blob_value BLOB NOT NULL,
            binary_value VARBINARY(4) NOT NULL,
            date_value DATE NOT NULL,
            time_value TIME NOT NULL,
            datetime_value DATETIME NOT NULL,
            timestamp_value TIMESTAMP NULL,
            year_value YEAR NOT NULL,
            enum_value ENUM('small', 'large') NOT NULL,
            set_value SET('red', 'blue') NOT NULL,
            json_value JSON NOT NULL,
            point_value POINT NOT NULL
        )",
        db
    ))
    .execute(&pool)
    .await
    .expect("create rare_types table");
    sqlx::query(&format!(
        "INSERT INTO `{}`.`rare_types`
            (id, bit_value, blob_value, binary_value, date_value, time_value, datetime_value,
             timestamp_value, year_value, enum_value, set_value, json_value, point_value)
         VALUES
            (1, b'1010', X'6869', X'00FF', '2024-02-03', '04:05:06',
             '2024-02-03 04:05:06', '2024-02-03 04:05:06', 2024, 'large',
             'red,blue', JSON_OBJECT('ok', true), ST_GeomFromText('POINT(1 2)'))",
        db
    ))
    .execute(&pool)
    .await
    .expect("insert rare types");

    let result = driver
        .get_table_data(
            &db,
            "rare_types",
            0,
            10,
            None,
            Some("id".into()),
            Some(false),
            true,
            None,
        )
        .await
        .expect("read rare types");
    let row = object_at(&result.rows, 0);

    assert_eq!(row["bit_value"], json!("1010"));
    assert_eq!(row["blob_value"], json!("hi"));
    assert_eq!(row["binary_value"], json!("0x00ff"));
    assert_eq!(row["date_value"], json!("2024-02-03"));
    assert_eq!(row["time_value"], json!("04:05:06"));
    assert_eq!(row["datetime_value"], json!("2024-02-03 04:05:06"));
    assert!(row["timestamp_value"]
        .as_str()
        .is_some_and(|value| value.starts_with("2024-02-03 04:05:06")));
    assert_eq!(row["year_value"], json!(2024));
    assert_eq!(row["enum_value"], json!("large"));
    assert_eq!(row["set_value"], json!("red,blue"));
    assert_eq!(row["json_value"], json!("{\"ok\": true}"));
    assert_eq!(row["point_value"], json!("POINT(1 2)"));

    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn execute_query_streams_select_results_in_chunks() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = MySqlDriver::new(pool.clone(), false);

    sqlx::query(&format!(
        "CREATE TABLE `{}`.`streamed` (id INT PRIMARY KEY, label VARCHAR(32) NOT NULL)",
        db
    ))
    .execute(&pool)
    .await
    .expect("create streamed table");

    let values = (1..=1205)
        .map(|i| format!("({}, 'row-{}')", i, i))
        .collect::<Vec<_>>()
        .join(", ");
    sqlx::query(&format!(
        "INSERT INTO `{}`.`streamed` (id, label) VALUES {}",
        db, values
    ))
    .execute(&pool)
    .await
    .expect("insert streamed rows");

    let chunk_sizes = Arc::new(Mutex::new(Vec::new()));
    let first_chunk_columns = Arc::new(Mutex::new(None));
    let chunk_sizes_for_cb = Arc::clone(&chunk_sizes);
    let columns_for_cb = Arc::clone(&first_chunk_columns);

    let result = driver
        .execute_query(
            Some(&db),
            "SELECT id, label FROM streamed ORDER BY id",
            Some("streaming-test"),
            None,
            Some(Arc::new(move |columns, rows| {
                if columns.is_some() {
                    *columns_for_cb.lock().unwrap() = columns;
                }
                chunk_sizes_for_cb.lock().unwrap().push(rows.len());
            })),
            None,
        )
        .await
        .expect("execute streaming query");

    assert!(result.is_select);
    assert_eq!(result.rows_affected, 1205);
    assert!(result.rows.is_empty());
    assert_eq!(*chunk_sizes.lock().unwrap(), vec![500, 500, 205]);
    let column_names = {
        let columns = first_chunk_columns.lock().unwrap();
        let columns = columns
            .as_ref()
            .expect("first chunk should include columns");
        columns
            .iter()
            .map(|c| c.name.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        column_names,
        vec!["id".to_string(), "label".to_string()]
    );
    assert!(driver.get_thread_id_for_query("streaming-test").is_none());

    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn cancel_query_kills_running_select_and_cleans_tracking() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = Arc::new(MySqlDriver::new(pool.clone(), false));
    let query_id = format!("cancel_{}", Uuid::new_v4().simple());

    let query_driver = Arc::clone(&driver);
    let query_db = db.clone();
    let query_id_for_task = query_id.clone();
    let handle = tokio::spawn(async move {
        query_driver
            .execute_query(
                Some(&query_db),
                "SELECT SLEEP(10) AS slept",
                Some(&query_id_for_task),
                None,
                None,
                None,
            )
            .await
    });

    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
    let thread_id = loop {
        if let Some(thread_id) = driver.get_thread_id_for_query(&query_id) {
            break thread_id;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "query was not registered before timeout"
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
    };

    driver
        .kill_query(thread_id)
        .await
        .expect("kill running query");

    let result = handle.await.expect("query task should not panic");
    assert!(result.is_err(), "cancelled query should return an error");
    assert!(driver.get_thread_id_for_query(&query_id).is_none());

    drop_database(&pool, &db).await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_MYSQL_URL pointing to a MySQL server"]
async fn abort_import_session_kills_running_batch_and_cleans_tracking() {
    let pool = test_pool().await;
    let db = setup_database(&pool).await;
    let driver = Arc::new(MySqlDriver::new(pool.clone(), false));
    let import_id = format!("import_cancel_{}", Uuid::new_v4().simple());

    driver
        .begin_import_session(&db, &import_id)
        .await
        .expect("begin import session");

    let import_driver = Arc::clone(&driver);
    let import_db = db.clone();
    let import_id_for_task = import_id.clone();
    let handle = tokio::spawn(async move {
        import_driver
            .execute_statements(
                &import_db,
                &["SELECT SLEEP(10)".to_string()],
                Some(&import_id_for_task),
            )
            .await
    });

    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
    let thread_id = loop {
        if let Some(thread_id) = driver.get_thread_id_for_import(&import_id) {
            break thread_id;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "import was not registered before timeout"
        );
        tokio::time::sleep(tokio::time::Duration::from_millis(25)).await;
    };

    driver
        .abort_import_session(&import_id)
        .await
        .expect("abort import session");
    driver
        .kill_connection(thread_id)
        .await
        .expect("kill import connection");

    let results = handle.await.expect("import task should not panic");
    assert_eq!(results.len(), 1);
    assert!(results[0].is_err(), "cancelled import batch should fail");
    assert!(driver.get_thread_id_for_import(&import_id).is_none());

    drop_database(&pool, &db).await;
}
