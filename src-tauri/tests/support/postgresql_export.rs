use super::*;
use app_lib::services::transfers::{export_database_file, import_sql_file, ExportOptions};

async fn export(
    db: &Database,
    path: &std::path::Path,
    mode: &str,
    tables: Option<Vec<TableRef>>,
    gzip: bool,
    drop_if_exists: bool,
) -> Result<usize, String> {
    export_database_file(
        db.driver.clone(),
        db.name.clone(),
        mode.into(),
        path.to_str().unwrap().into(),
        tables,
        "sql",
        ExportOptions {
            drop_if_exists,
            include_views: true,
            use_transactions: true,
            compress_gzip: gzip,
        },
        &|_| {},
        &|| false,
    )
    .await
}
async fn restore(db: &Database, path: &std::path::Path) {
    import_sql_file(
        db.driver.clone(),
        &db.name,
        path.to_str().unwrap(),
        "restore-export",
        &|| false,
        &|_| {},
    )
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn regression_domain_defaults_resolve_renamed_schema_dependencies() {
    let source = Database::new().await;
    source.seed(r#"CREATE SCHEMA "Default Types";
        SET search_path = "Default Types", pg_catalog;
        CREATE TYPE "Mood" AS ENUM ('ok','no');
        CREATE DOMAIN "MoodDomain" AS "Mood" DEFAULT 'ok';
        CREATE FUNCTION "DefaultText"() RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT 'domain default'::text $$;
        CREATE DOMAIN "TextDomain" AS text DEFAULT "DefaultText"();
        CREATE SEQUENCE "DomainSeq" START WITH 101;
        CREATE DOMAIN "IdDomain" AS bigint DEFAULT nextval('"DomainSeq"'::regclass);
        CREATE DOMAIN "NullableDomain" AS integer DEFAULT NULL;
        CREATE TABLE public.domain_values(id "IdDomain", mood "MoodDomain", label "TextDomain", optional "NullableDomain");
        INSERT INTO public.domain_values DEFAULT VALUES;
        ALTER TYPE "Mood" RENAME TO "RenamedMood";
        ALTER FUNCTION "DefaultText"() RENAME TO "RenamedDefaultText";
        ALTER SEQUENCE "DomainSeq" RENAME TO "RenamedDomainSeq";
        RESET search_path;
        CREATE TABLE public.unrelated(id integer)"#).await;
    let target = Database::new().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("domain-defaults.sql");
    for (mode, selection) in [
        ("full", Some(vec![source.table("public", "domain_values")])),
        (
            "structure",
            Some(vec![source.table("public", "domain_values")]),
        ),
        ("full", None),
    ] {
        export(&source, &path, mode, selection, false, true)
            .await
            .unwrap();
        restore(&target, &path).await;
        restore(&target, &path).await;
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM public.domain_values")
            .fetch_one(&target.pool)
            .await
            .unwrap();
        assert_eq!(rows, if mode == "full" { 1 } else { 0 });
        let inserted: (i64, String, String, Option<i32>) = sqlx::query_as(
            "INSERT INTO public.domain_values DEFAULT VALUES RETURNING id::bigint, mood::text, label::text, optional::integer"
        ).fetch_one(&target.pool).await.unwrap();
        assert_eq!(
            inserted,
            (
                if mode == "full" { 102 } else { 101 },
                "ok".into(),
                "domain default".into(),
                None
            )
        );
    }
    source.close().await;
    target.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn regression_view_defaults_restore_values_and_dependency_order() {
    let source = Database::new().await;
    source.seed(r#"CREATE SCHEMA "View Space";
        SET search_path = "View Space", pg_catalog;
        CREATE TYPE "Mood" AS ENUM ('ok','no');
        CREATE SEQUENCE "ViewSeq" START WITH 100;
        CREATE TABLE base("row id" bigint DEFAULT 1, token bigint DEFAULT 1, mood "Mood" DEFAULT 'no', label text DEFAULT 'base', optional integer DEFAULT 9);
        CREATE VIEW "write view" AS SELECT * FROM base;
        CREATE FUNCTION "ViewLabel"(v "write view") RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT 'view default'::text $$;
        ALTER VIEW "write view" ALTER COLUMN "row id" SET DEFAULT 42;
        ALTER VIEW "write view" ALTER COLUMN token SET DEFAULT nextval('"ViewSeq"'::regclass);
        ALTER VIEW "write view" ALTER COLUMN mood SET DEFAULT 'ok';
        ALTER VIEW "write view" ALTER COLUMN label SET DEFAULT "ViewLabel"(NULL::"write view");
        ALTER VIEW "write view" ALTER COLUMN optional SET DEFAULT NULLIF(9,9);
        INSERT INTO "write view" DEFAULT VALUES;
        RESET search_path;
        CREATE TABLE public.unrelated(id integer)"#).await;
    let target = Database::new().await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("view-defaults.sql");
    let insert = r#"INSERT INTO "View Space"."write view" DEFAULT VALUES RETURNING "row id", token, mood::text, label, optional"#;
    let original: (i64, i64, String, String, Option<i32>) = sqlx::query_as(
        r#"SELECT "row id", token, mood::text, label, optional FROM "View Space"."write view""#,
    )
    .fetch_one(&source.pool)
    .await
    .unwrap();
    assert_eq!(
        original,
        (42, 100, "ok".into(), "view default".into(), None)
    );
    for (mode, selection) in [
        (
            "full",
            Some(vec![
                source.table("View Space", "base"),
                source.table("View Space", "write view"),
            ]),
        ),
        (
            "structure",
            Some(vec![
                source.table("View Space", "base"),
                source.table("View Space", "write view"),
            ]),
        ),
        ("full", None),
    ] {
        export(&source, &path, mode, selection, false, true)
            .await
            .unwrap();
        restore(&target, &path).await;
        restore(&target, &path).await;
        let inserted: (i64, i64, String, String, Option<i32>) = sqlx::query_as(insert)
            .fetch_one(&target.pool)
            .await
            .unwrap();
        assert_eq!(
            inserted,
            (
                42,
                if mode == "full" { 101 } else { 100 },
                "ok".into(),
                "view default".into(),
                None
            )
        );
        let count: i64 = sqlx::query_scalar(r#"SELECT count(*) FROM "View Space"."write view""#)
            .fetch_one(&target.pool)
            .await
            .unwrap();
        assert_eq!(count, if mode == "full" { 2 } else { 1 });
    }
    source.close().await;
    target.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn regression_inspected_view_ddl_preserves_column_defaults() {
    let db = Database::new().await;
    db.seed(
        r#"CREATE TABLE public.base(id integer DEFAULT 1, label text DEFAULT 'base');
        CREATE VIEW public."edit view" AS SELECT id AS "row id", label FROM public.base;
        ALTER VIEW public."edit view" ALTER COLUMN "row id" SET DEFAULT 42;
        ALTER VIEW public."edit view" ALTER COLUMN label SET DEFAULT NULLIF('view','view')"#,
    )
    .await;
    // A bare SET DEFAULT NULL is removed by PostgreSQL itself. This expression
    // remains a real view default and must override the base table's default.
    let original: (i32, Option<String>) = sqlx::query_as(
        r#"INSERT INTO public."edit view" DEFAULT VALUES RETURNING "row id", label"#,
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(original, (42, None));
    let ddl = db
        .driver
        .get_table_ddl(&db.table("public", "edit view"))
        .await
        .unwrap();
    db.seed(r#"DROP VIEW public."edit view""#).await;
    db.seed(&ddl).await;
    let inserted: (i32, Option<String>) = sqlx::query_as(
        r#"INSERT INTO public."edit view" DEFAULT VALUES RETURNING "row id", label"#,
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(inserted, (42, None));
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn regression_identity_partitions_share_root_sequence_and_do_not_break_other_ddl() {
    let source = Database::new().await;
    source.seed("CREATE SCHEMA pgapp;
        CREATE TABLE pgapp.events(id bigint GENERATED ALWAYS AS IDENTITY (START WITH 10 INCREMENT BY 2), bucket int, label text, PRIMARY KEY(id,bucket)) PARTITION BY RANGE(bucket);
        CREATE TABLE pgapp.events_a PARTITION OF pgapp.events FOR VALUES FROM(0) TO(100) PARTITION BY RANGE(bucket);
        CREATE TABLE pgapp.events_a1 PARTITION OF pgapp.events_a FOR VALUES FROM(0) TO(50);
        CREATE TABLE pgapp.events_a2 PARTITION OF pgapp.events_a FOR VALUES FROM(50) TO(100);
        CREATE TABLE pgapp.events_other PARTITION OF pgapp.events DEFAULT;
        INSERT INTO pgapp.events(bucket,label) VALUES(1,'one'),(51,'two'),(101,'other');
        CREATE TABLE public.unrelated(id integer PRIMARY KEY)").await;
    for table in [
        source.table("public", "unrelated"),
        source.table("pgapp", "events"),
        source.table("pgapp", "events_a"),
        source.table("pgapp", "events_a1"),
    ] {
        assert!(source
            .driver
            .get_table_ddl(&table)
            .await
            .unwrap()
            .contains("CREATE TABLE"));
    }
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("identity.sql");
    let target = Database::new().await;
    let version: i32 = sqlx::query_scalar("SELECT current_setting('server_version_num')::integer")
        .fetch_one(&source.pool)
        .await
        .unwrap();
    for selection in [Some(vec![source.table("pgapp", "events")]), None] {
        assert_eq!(
            export(&source, &path, "full", selection, false, true)
                .await
                .unwrap(),
            3
        );
        restore(&target, &path).await;
        restore(&target, &path).await;
        let rows: Vec<(i64, i32, String)> =
            sqlx::query_as("SELECT id,bucket,label FROM pgapp.events ORDER BY id")
                .fetch_all(&target.pool)
                .await
                .unwrap();
        assert_eq!(
            rows,
            vec![
                (10, 1, "one".into()),
                (12, 51, "two".into()),
                (14, 101, "other".into())
            ]
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("INSERT INTO pgapp.events(bucket) VALUES(2) RETURNING id")
                .fetch_one(&target.pool)
                .await
                .unwrap(),
            16
        );
        // PostgreSQL 17+ inherits identity on partitions, including direct leaf
        // inserts. Older servers need an explicit value when bypassing the root.
        let leaf_identity: String = sqlx::query_scalar("SELECT attidentity::text FROM pg_attribute WHERE attrelid='pgapp.events_a1'::regclass AND attname='id'")
            .fetch_one(&target.pool).await.unwrap();
        assert_eq!(leaf_identity, if version >= 170000 { "a" } else { "" });
        assert_eq!(
            sqlx::query_scalar::<_, i64>(if version >= 170000 {
                "INSERT INTO pgapp.events_a1(bucket) VALUES(3) RETURNING id"
            } else {
                "INSERT INTO pgapp.events_a1(id,bucket) VALUES(18,3) RETURNING id"
            })
            .fetch_one(&target.pool)
            .await
            .unwrap(),
            18
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("INSERT INTO pgapp.events(bucket) VALUES(4) RETURNING id")
                .fetch_one(&target.pool)
                .await
                .unwrap(),
            if version >= 170000 { 20 } else { 18 }
        );
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT count(*) FROM pg_sequence s JOIN pg_class c ON c.oid=s.seqrelid JOIN pg_namespace n ON n.oid=c.relnamespace WHERE n.nspname='pgapp'").fetch_one(&target.pool).await.unwrap(),1);
    }
    source.close().await;
    target.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn export_restores_custom_collations_and_nested_partition_indexes() {
    let source = Database::new().await;
    source.seed("CREATE SCHEMA custom;
        CREATE COLLATION custom.letters (provider=libc,lc_collate='C',lc_ctype='C');
        CREATE COLLATION custom.caseless (provider=icu,locale='und-u-ks-level2',deterministic=false);
        COMMENT ON COLLATION custom.letters IS 'portable letters';
        CREATE DOMAIN custom.label AS text COLLATE custom.letters;
        CREATE TABLE public.events(id integer, label custom.label, lookup text COLLATE custom.caseless) PARTITION BY RANGE(id);
        CREATE TABLE public.events_a PARTITION OF public.events FOR VALUES FROM(0) TO(100) PARTITION BY RANGE(id);
        CREATE TABLE public.events_a1 PARTITION OF public.events_a FOR VALUES FROM(0) TO(50);
        CREATE TABLE public.events_a2 PARTITION OF public.events_a FOR VALUES FROM(50) TO(100);
        CREATE TABLE public.events_b PARTITION OF public.events FOR VALUES FROM(100) TO(200);
        CREATE INDEX events_label ON public.events(lower(label)) INCLUDE(id) WHERE id>0;
        COMMENT ON INDEX public.events_label IS 'partition index';
        INSERT INTO public.events VALUES(1,'A','Hello'),(51,'B','World'),(101,'C','Other');").await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("partitions.sql");
    assert_eq!(
        export(&source, &path, "full", None, false, true)
            .await
            .unwrap(),
        3
    );
    let script = std::fs::read_to_string(&path).unwrap();
    assert!(script.contains("CREATE COLLATION"));
    assert!(script.contains("ATTACH PARTITION"));
    assert!(!script.contains("CREATE ROLE"));
    assert!(!script.contains("OWNER TO"));
    let target = Database::new().await;
    restore(&target, &path).await;
    restore(&target, &path).await;
    let index_query = "SELECT c.relname::text,i.indisvalid,COALESCE(p.relname::text,'') FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid JOIN pg_namespace n ON n.oid=c.relnamespace LEFT JOIN pg_inherits h ON h.inhrelid=c.oid LEFT JOIN pg_class p ON p.oid=h.inhparent WHERE n.nspname='public' ORDER BY c.relname";
    let a: Vec<(String, bool, String)> = sqlx::query_as(index_query)
        .fetch_all(&source.pool)
        .await
        .unwrap();
    let b: Vec<(String, bool, String)> = sqlx::query_as(index_query)
        .fetch_all(&target.pool)
        .await
        .unwrap();
    assert_eq!(a, b);
    assert!(a.iter().all(|(_, valid, _)| *valid));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM public.events WHERE lookup='hello'")
            .fetch_one(&target.pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "SELECT obj_description('custom.letters'::regcollation,'pg_collation')"
        )
        .fetch_one(&target.pool)
        .await
        .unwrap(),
        "portable letters"
    );
    source.close().await;
    target.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn export_full_type_fixture_restores_every_table_and_value() {
    let source = Database::new().await;
    source
        .seed(include_str!(
            "../../../scripts/fixtures/postgresql_test_data.sql"
        ))
        .await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fixture.sql");
    let count = export(&source, &path, "full", None, false, true)
        .await
        .unwrap();
    assert!(count >= 5000);
    let target = Database::new().await;
    restore(&target, &path).await;
    let source_tables = source.driver.get_tables(&source.name).await.unwrap();
    let target_tables = target.driver.get_tables(&target.name).await.unwrap();
    assert_eq!(source_tables.len(), target_tables.len());
    for t in &source_tables {
        // Object OIDs, reg* references and sequence relation OIDs are intentionally
        // database-local. All other values must survive exactly as SQL text.
        if t.name == "system_types" {
            continue;
        }
        let qualified = format!(
            "\"{}\".\"{}\"",
            t.reference.schema.as_ref().unwrap().replace('"', "\"\""),
            t.name.replace('"', "\"\"")
        );
        let sql =
            format!("SELECT row_to_json(t)::text FROM {qualified} t ORDER BY row_to_json(t)::text");
        let a: Vec<String> = sqlx::query_scalar(&sql)
            .fetch_all(&source.pool)
            .await
            .unwrap();
        let b: Vec<String> = sqlx::query_scalar(&sql)
            .fetch_all(&target.pool)
            .await
            .unwrap();
        assert_eq!(a, b, "{qualified}");
    }
    // Restoring with Drop-if-exists twice validates reverse dependency ordering.
    restore(&target, &path).await;
    let next: i32 = sqlx::query_scalar(
        "INSERT INTO tupledb_test.users(name,email) VALUES('new','new@example.test') RETURNING id",
    )
    .fetch_one(&target.pool)
    .await
    .unwrap();
    let source_next: i32 = sqlx::query_scalar(
        "INSERT INTO tupledb_test.users(name,email) VALUES('new','new@example.test') RETURNING id",
    )
    .fetch_one(&source.pool)
    .await
    .unwrap();
    assert_eq!(next, source_next);
    target.close().await;
    source.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn export_functions_triggers_sequence_options_and_schema_data_modes() {
    let source = Database::new().await;
    source.seed("CREATE SCHEMA app; CREATE TYPE app.status AS ENUM ('a','b'); CREATE TABLE app.items(id bigint GENERATED ALWAYS AS IDENTITY (START WITH 50 INCREMENT BY 3 CACHE 2), status app.status DEFAULT 'a', note text, computed text GENERATED ALWAYS AS (upper(note)) STORED, PRIMARY KEY(id)); CREATE FUNCTION app.audit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN NEW.note := NEW.note || '!'; RETURN NEW; END $$; CREATE TRIGGER audit BEFORE INSERT ON app.items FOR EACH ROW EXECUTE FUNCTION app.audit(); INSERT INTO app.items(note) VALUES('hello'); COMMENT ON TABLE app.items IS 'custom comment'; COMMENT ON COLUMN app.items.note IS 'text note'; CREATE TABLE app.other(id int); CREATE VIEW app.v AS SELECT * FROM app.items;").await;
    let dir = tempfile::tempdir().unwrap();
    let schema = dir.path().join("schema.sql");
    let data = dir.path().join("data.sql");
    assert_eq!(
        export(&source, &schema, "structure", None, false, false)
            .await
            .unwrap(),
        0
    );
    export(&source, &data, "data", None, false, false)
        .await
        .unwrap();
    let target = Database::new().await;
    restore(&target, &schema).await;
    let n: i64 = sqlx::query_scalar("SELECT count(*) FROM app.items")
        .fetch_one(&target.pool)
        .await
        .unwrap();
    assert_eq!(n, 0);
    // Data-only restores leave existing triggers enabled, exactly like manual COPY.
    target
        .seed("ALTER TABLE app.items DISABLE TRIGGER audit")
        .await;
    restore(&target, &data).await;
    target
        .seed("ALTER TABLE app.items ENABLE TRIGGER audit")
        .await;
    let row: (i64, String, String) = sqlx::query_as("SELECT id,note,computed FROM app.items")
        .fetch_one(&target.pool)
        .await
        .unwrap();
    assert_eq!(row, (50, "hello!".into(), "HELLO!".into()));
    let next: (i64, String) =
        sqlx::query_as("INSERT INTO app.items(note) VALUES('after') RETURNING id,note")
            .fetch_one(&target.pool)
            .await
            .unwrap();
    assert!(next.0 > 50);
    assert_eq!(next.1, "after!");
    let comment: String =
        sqlx::query_scalar("SELECT obj_description('app.items'::regclass,'pg_class')")
            .fetch_one(&target.pool)
            .await
            .unwrap();
    assert_eq!(comment, "custom comment");
    target.close().await;
    source.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn export_selection_errors_cancellation_and_gzip_preserve_existing_files() {
    use std::{
        io::Read,
        sync::atomic::{AtomicBool, Ordering},
    };
    let db = Database::new().await;
    db.seed("CREATE TABLE parent(id int PRIMARY KEY); CREATE TABLE child(id int REFERENCES parent); INSERT INTO parent VALUES(1); INSERT INTO child VALUES(1); CREATE TABLE private(id int); ALTER TABLE private ENABLE ROW LEVEL SECURITY;").await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("export.sql");
    std::fs::write(&path, "keep me").unwrap();
    let err = export(
        &db,
        &path,
        "full",
        Some(vec![db.table("public", "child")]),
        false,
        true,
    )
    .await
    .unwrap_err();
    assert!(err.contains("Include that table"), "{err}");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep me");
    let err = export(
        &db,
        &path,
        "full",
        Some(vec![db.table("public", "private")]),
        false,
        true,
    )
    .await
    .unwrap_err();
    assert!(err.contains("row security"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep me");
    let cancel = AtomicBool::new(false);
    let err = export_database_file(
        db.driver.clone(),
        db.name.clone(),
        "full".into(),
        path.to_str().unwrap().into(),
        Some(vec![db.table("public", "parent")]),
        "sql",
        ExportOptions::default(),
        &|_| {
            cancel.store(true, Ordering::SeqCst);
        },
        &|| cancel.load(Ordering::SeqCst),
    )
    .await
    .unwrap_err();
    assert!(err.contains("cancelled"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep me");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    export(
        &db,
        &path,
        "full",
        Some(vec![
            db.table("public", "parent"),
            db.table("public", "child"),
        ]),
        true,
        true,
    )
    .await
    .unwrap();
    let mut sql = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(&path).unwrap())
        .read_to_string(&mut sql)
        .unwrap();
    assert!(sql.contains("COPY \"public\".\"child\""));
    assert!(sql.contains("FOREIGN KEY"));
    let target = Database::new().await;
    target
        .driver
        .import_stream(
            &target.name,
            &mut std::io::Cursor::new(sql.as_bytes()),
            sql.len(),
            "gzip-restore",
            &|| false,
            &|_, _, _| {},
        )
        .await
        .unwrap();
    target.close().await;
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn export_partition_selection_and_single_table_data_on_read_only_connection() {
    let source = Database::new().await;
    source.seed("CREATE TABLE events(id int PRIMARY KEY) PARTITION BY RANGE(id); CREATE TABLE events_a PARTITION OF events FOR VALUES FROM(0) TO(10); CREATE TABLE events_b PARTITION OF events DEFAULT; INSERT INTO events VALUES(1),(20); CREATE TABLE unrelated(id int); INSERT INTO unrelated VALUES(99);").await;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("partitions.sql");
    assert_eq!(
        export(
            &source,
            &path,
            "full",
            Some(vec![source.table("public", "events")]),
            false,
            true
        )
        .await
        .unwrap(),
        2
    );
    let target = Database::new().await;
    restore(&target, &path).await;
    let ids: Vec<i32> = sqlx::query_scalar("SELECT id FROM events ORDER BY id")
        .fetch_all(&target.pool)
        .await
        .unwrap();
    assert_eq!(ids, vec![1, 20]);
    let unrelated: bool = sqlx::query_scalar("SELECT to_regclass('public.unrelated') IS NOT NULL")
        .fetch_one(&target.pool)
        .await
        .unwrap();
    assert!(!unrelated);
    let readonly = Arc::new(PostgreSqlDriver::new(
        source.pool.clone(),
        source.name.clone(),
        true,
    ));
    let path = dir.path().join("data.sql");
    let count = app_lib::services::transfers::export_table_file(
        readonly,
        source.name.clone(),
        source.table("public", "unrelated"),
        "sql".into(),
        path.to_str().unwrap().into(),
        &|_, _, _| {},
    )
    .await
    .unwrap();
    assert_eq!(count, 1);
    target.seed("CREATE TABLE unrelated(id int)").await;
    restore(&target, &path).await;
    let id: i32 = sqlx::query_scalar("SELECT id FROM unrelated")
        .fetch_one(&target.pool)
        .await
        .unwrap();
    assert_eq!(id, 99);
    target.close().await;
    source.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn export_cancellation_interrupts_a_server_lock_without_publishing() {
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };
    let db = Database::new().await;
    db.seed("CREATE TABLE locked(id int); INSERT INTO locked VALUES(1)")
        .await;
    let mut blocker = db.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE locked IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("existing.sql");
    std::fs::write(&path, "keep me").unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = cancel.clone();
    let driver = db.driver.clone();
    let database = db.name.clone();
    let output = path.clone();
    let worker = tokio::spawn(async move {
        export_database_file(
            driver,
            database,
            "full".into(),
            output.to_str().unwrap().into(),
            None,
            "sql",
            ExportOptions::default(),
            &|_| {},
            &|| worker_cancel.load(Ordering::SeqCst),
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(10),async {
        loop {
            let waiting:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=$1 AND wait_event_type='Lock' AND query LIKE 'LOCK TABLE%')").bind(&db.name).fetch_one(&db.admin).await.unwrap();
            if waiting {break;} tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
    cancel.store(true, Ordering::SeqCst);
    let error = tokio::time::timeout(Duration::from_secs(8), worker)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(error.contains("cancelled"), "{error}");
    blocker.rollback().await.unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "keep me");
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    db.close().await;
}
