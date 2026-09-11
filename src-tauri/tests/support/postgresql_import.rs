use super::*;
use std::{io::Cursor, time::Duration};

async fn import_text(db: &Database, sql: &str) -> Result<ImportResult, String> {
    db.driver
        .import_stream(
            &db.name,
            &mut Cursor::new(sql.as_bytes()),
            sql.len(),
            "test-import",
            &|| false,
            &|_, _, _| {},
        )
        .await
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn import_sql_fixture_through_file_service() {
    let db = Database::new().await;
    let progress = Mutex::new(Vec::new());
    let result = app_lib::services::transfers::import_sql_file(
        db.driver.clone(),
        &db.name,
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../scripts/fixtures/postgresql_test_data.sql"
        ),
        "fixture",
        &|| false,
        &|p| progress.lock().unwrap().push(p),
    )
    .await
    .unwrap();
    assert_eq!(result.executed, 1);
    assert!(result.errors.is_empty());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM tupledb_test.pagination")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 5000);
    {
        let updates = progress.lock().unwrap();
        let last = updates.last().unwrap();
        assert_eq!(last.current, last.total);
        assert!(last.status.starts_with("Imported"));
    }
    assert!(db
        .driver
        .get_tables(&db.name)
        .await
        .unwrap()
        .iter()
        .any(|t| t.reference.schema.as_deref() == Some("tupledb_test_alt")));
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn import_copy_text_csv_functions_and_session_isolation() {
    let db = Database::new().await;
    let mut script = String::from("\u{feff}-- plain dump\r\n\\restrict abc123\r\nBEGIN;\r\nSET search_path = public;\nCREATE TABLE imported(id int PRIMARY KEY, value text);\nCREATE FUNCTION imported_f() RETURNS text LANGUAGE plpgsql AS $body$ BEGIN RETURN 'a;b'; END; $body$;\nCOPY imported (id, value) FROM stdin;\n1\tUnicode 🐘\\tand\\nnewline\n2\t\\N\n3\t\\\\.\n");
    for i in 4..10004 {
        script.push_str(&format!("{i}\trow {i}; not SQL\n"));
    }
    script.push_str("\\.\nCREATE TABLE imported_csv(id int, value text);\nCOPY imported_csv FROM STDIN WITH (FORMAT csv);\n1,\"two\nlines; kept\"\n2,\"a,b\"\n\\.\nINSERT INTO imported VALUES(20000, imported_f()); COMMIT;\n\\unrestrict abc123\n");
    let result = import_text(&db, &script).await.unwrap();
    assert_eq!(result.executed, 7);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM imported")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(count, 10004);
    let value: String = sqlx::query_scalar("SELECT value FROM imported WHERE id=1")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(value, "Unicode 🐘\tand\nnewline");
    let null: Option<String> = sqlx::query_scalar("SELECT value FROM imported WHERE id=2")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(null.is_none());
    let csv: String = sqlx::query_scalar("SELECT value FROM imported_csv WHERE id=1")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(csv, "two\nlines; kept");
    let search_path: String = sqlx::query_scalar("SHOW search_path")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert_eq!(search_path, "\"$user\", public");
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn import_errors_rollback_prior_work_and_reject_unsupported_formats() {
    let db = Database::new().await;
    db.seed("CREATE TABLE existing(id int PRIMARY KEY); INSERT INTO existing VALUES(1)")
        .await;
    for suffix in [
        "INSERT INTO existing VALUES(1);",
        "SELECT 'unterminated",
        "COPY created FROM STDIN;\n2\n", // no terminator
        "COPY created FROM STDIN;\ninvalid-int\n\\.\n",
        "\\connect another_database\n",
        "\\restrict abc\nSELECT 1;\n\\unrestrict wrong\n",
        "\\restrict abc\nSELECT 1;\n",
        "COMMIT AND CHAIN;",
        "COMMIT (bad);",
        "ROLLBACK;",
        "SET standard_conforming_strings = off;",
        "SELECT set_config('standard_conforming_strings', 'off', false);",
        "COPY created TO STDOUT;",
        "COPY created FROM PROGRAM 'echo 1';",
        "COPY created FROM STDIN WITH (FORMAT binary);\n\\.\n",
    ] {
        let script = format!("BEGIN; CREATE TABLE created(id int); INSERT INTO existing VALUES(2); COMMIT;\n{suffix}");
        let error = import_text(&db, &script).await.expect_err(suffix);
        assert!(error.contains("not committed"), "{error}");
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM existing")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert_eq!(count, 1, "{suffix}");
        let exists: bool = sqlx::query_scalar("SELECT to_regclass('public.created') IS NOT NULL")
            .fetch_one(&db.pool)
            .await
            .unwrap();
        assert!(!exists, "{suffix}");
    }
    for input in [
        b"PGDMP archive".as_slice(),
        &[0x1f, 0x8b, 0xff],
        &[0xff, 0xfe],
    ] {
        assert!(db
            .driver
            .import_stream(
                &db.name,
                &mut Cursor::new(input),
                input.len(),
                "test-import",
                &|| false,
                &|_, _, _| {}
            )
            .await
            .is_err());
    }
    import_text(&db, "INSERT INTO existing VALUES(3)")
        .await
        .unwrap();
    let readonly = PostgreSqlDriver::new(db.pool.clone(), db.name.clone(), true);
    assert!(readonly
        .import_stream(
            &db.name,
            &mut Cursor::new(b"DROP TABLE existing"),
            19,
            "ro",
            &|| false,
            &|_, _, _| {}
        )
        .await
        .is_err());
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn import_cancellation_interrupts_active_sql_and_rolls_back() {
    let db = Database::new().await;
    let single = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(db.pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    let driver = Arc::new(PostgreSqlDriver::new(single, db.name.clone(), false));
    let worker_driver = driver.clone();
    let database = db.name.clone();
    let worker = tokio::spawn(async move {
        let script = "CREATE TABLE canceled_import(id int); SELECT pg_sleep(30) /* import-cancel-test */; INSERT INTO canceled_import VALUES(1);";
        worker_driver
            .import_stream(
                &database,
                &mut Cursor::new(script.as_bytes()),
                script.len(),
                "cancel-me",
                &|| false,
                &|_, _, _| {},
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=$1 AND state='active' AND query LIKE ' SELECT pg_sleep(30)%')").bind(&db.name).fetch_one(&db.admin).await.unwrap();
            if active { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.expect("sleep query never started");
    driver.cancel_import("cancel-me").await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(8), worker)
        .await
        .unwrap()
        .unwrap();
    assert!(result.unwrap_err().contains("Import cancelled"));
    let exists: bool =
        sqlx::query_scalar("SELECT to_regclass('public.canceled_import') IS NOT NULL")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert!(!exists);
    let active: i64 = sqlx::query_scalar("SELECT count(*) FROM pg_stat_activity WHERE datname=$1 AND state='active' AND query LIKE ' SELECT pg_sleep(30)%'").bind(&db.name).fetch_one(&db.admin).await.unwrap();
    assert_eq!(active, 0);
    driver.close().await;
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL and TUPLEDB_TEST_PG_DUMP_CONTAINER"]
async fn import_real_pg_dump_copy_and_insert_formats() {
    let container =
        std::env::var("TUPLEDB_TEST_PG_DUMP_CONTAINER").expect("Set isolated test container name");
    let source = Database::new().await;
    source.seed("CREATE SCHEMA dump_test; CREATE TABLE dump_test.items(id serial PRIMARY KEY, label text, payload bytea); INSERT INTO dump_test.items(label, payload) VALUES(E'quote '' ; \\n 🐘', decode('00ff', 'hex')), (NULL, NULL); CREATE FUNCTION dump_test.f() RETURNS text LANGUAGE plpgsql AS $$ BEGIN RETURN 'a;b'; END; $$;").await;
    for inserts in [false, true] {
        let mut command = std::process::Command::new("docker");
        command.args([
            "exec",
            &container,
            "pg_dump",
            "-U",
            "postgres",
            "--no-owner",
            "--no-privileges",
            "--schema=dump_test",
        ]);
        if inserts {
            command.arg("--inserts");
        }
        let output = command.arg(&source.name).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let dump = String::from_utf8(output.stdout).unwrap();
        assert!(dump.contains(if inserts {
            "INSERT INTO"
        } else {
            "COPY dump_test.items"
        }));
        let target = Database::new().await;
        import_text(&target, &dump).await.unwrap();
        let rows: Vec<(i32, Option<String>, Option<Vec<u8>>)> =
            sqlx::query_as("SELECT * FROM dump_test.items ORDER BY id")
                .fetch_all(&target.pool)
                .await
                .unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].1.as_deref(), Some("quote ' ; \n 🐘"));
        assert_eq!(rows[0].2, Some(vec![0, 255]));
        let id: i32 = sqlx::query_scalar(
            "INSERT INTO dump_test.items(label) VALUES(dump_test.f()) RETURNING id",
        )
        .fetch_one(&target.pool)
        .await
        .unwrap();
        assert_eq!(id, 3);
        target.close().await;
    }
    source.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn import_cancel_during_copy_and_disconnect_during_sql() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let db = Database::new().await;
    let mut script =
        String::from("CREATE TABLE canceled_copy(id int);\nCOPY canceled_copy FROM STDIN;\n");
    for i in 0..10000 {
        script.push_str(&format!("{i}\n"));
    }
    script.push_str("\\.\n");
    let checks = AtomicUsize::new(0);
    let error = db
        .driver
        .import_stream(
            &db.name,
            &mut Cursor::new(script.as_bytes()),
            script.len(),
            "copy-cancel",
            &|| checks.fetch_add(1, Ordering::SeqCst) > 100,
            &|_, _, _| {},
        )
        .await
        .unwrap_err();
    assert!(error.contains("Import cancelled"));
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('public.canceled_copy') IS NOT NULL")
        .fetch_one(&db.pool)
        .await
        .unwrap();
    assert!(!exists);

    let driver = db.driver.clone();
    let database = db.name.clone();
    let worker = tokio::spawn(async move {
        let script = "CREATE TABLE disconnected_import(id int); SELECT pg_sleep(30) /* disconnect-import */;";
        driver
            .import_stream(
                &database,
                &mut Cursor::new(script.as_bytes()),
                script.len(),
                "disconnect",
                &|| false,
                &|_, _, _| {},
            )
            .await
    });
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=$1 AND state='active' AND query LIKE ' SELECT pg_sleep(30)%')").bind(&db.name).fetch_one(&db.admin).await.unwrap();
            if active { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    tokio::time::timeout(Duration::from_secs(8), db.driver.close())
        .await
        .unwrap();
    assert!(worker
        .await
        .unwrap()
        .unwrap_err()
        .contains("Import cancelled"));
    db.close().await;
}
