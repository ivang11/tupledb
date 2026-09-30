use app_lib::database::driver::*;
use app_lib::database::drivers::postgresql::PostgreSqlDriver;
use app_lib::filters::{FilterRow, FilterSet, Operator};
use serde_json::{json, Value};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

#[path = "support/postgresql_completion.rs"]
mod completion_cases;
#[path = "support/postgresql_export.rs"]
mod export_cases;
#[path = "support/postgresql_import.rs"]
mod import_cases;

struct Database {
    admin: PgPool,
    pool: PgPool,
    driver: Arc<PostgreSqlDriver>,
    name: String,
}
impl Database {
    async fn new() -> Self {
        let url = std::env::var("TUPLEDB_TEST_POSTGRESQL_URL")
            .expect("Set TUPLEDB_TEST_POSTGRESQL_URL to an isolated PostgreSQL server");
        let opts = PgConnectOptions::from_str(&url).unwrap();
        let admin = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(opts.clone())
            .await
            .unwrap();
        let name = format!("tupledb_it_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("CREATE DATABASE {name}"))
            .execute(&admin)
            .await
            .unwrap();
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect_with(opts.database(&name))
            .await
            .unwrap();
        let driver = Arc::new(PostgreSqlDriver::new(pool.clone(), name.clone(), false));
        Self {
            admin,
            pool,
            driver,
            name,
        }
    }
    fn table(&self, schema: &str, name: &str) -> TableRef {
        TableRef {
            catalog: self.name.clone(),
            schema: Some(schema.into()),
            name: name.into(),
        }
    }
    async fn seed(&self, sql: &str) {
        sqlx::raw_sql(sql).execute(&self.pool).await.unwrap();
    }
    async fn close(self) {
        self.driver.close().await;
        // SQLx may still be completing a background connection release. This
        // fixture owns the randomly named database and all of its sessions.
        sqlx::query(&format!("DROP DATABASE {} WITH (FORCE)", self.name))
            .execute(&self.admin)
            .await
            .unwrap();
        self.admin.close().await;
    }
}
fn change(column: &str, value: Value) -> TableChange {
    TableChange {
        column: column.into(),
        value,
    }
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn one_saved_connection_routes_catalogs_and_creates_databases_safely() {
    let db = Database::new().await;
    let other = format!("{} .\"other", db.name);
    db.driver.create_database(&other, None, None).await.unwrap();
    let databases = db.driver.get_databases().await.unwrap();
    assert!(databases.contains(&db.name));
    assert!(databases.contains(&other));
    assert!(!databases.contains(&"template0".into()));
    assert!(!databases.contains(&"template1".into()));
    assert!(db.driver.capabilities().create_database);
    assert!(db.driver.capabilities().database_collations);
    assert!(db.driver.create_database("", None, None).await.is_err());
    assert!(db
        .driver
        .create_database("unsupported", Some("utf8"), None)
        .await
        .is_err());
    db.seed("CREATE TABLE public.users(id int PRIMARY KEY, label text NOT NULL); INSERT INTO public.users VALUES(1,'initial')").await;
    db.driver
        .execute_query(
            Some(&other),
            "CREATE TABLE public.users(id int PRIMARY KEY, label text NOT NULL)",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let target = TableRef {
        catalog: other.clone(),
        schema: Some("public".into()),
        name: "users".into(),
    };
    db.driver
        .insert_row(
            &target,
            vec![change("id", json!(1)), change("label", json!("other"))],
            false,
        )
        .await
        .unwrap();
    let initial = db.table("public", "users");
    let (a, b) = tokio::join!(
        db.driver
            .get_table_data(&initial, 0, 10, None, None, None, true, None),
        db.driver
            .get_table_data(&target, 0, 10, None, None, None, true, None)
    );
    assert_eq!(a.unwrap().rows[0]["label"], "initial");
    assert_eq!(b.unwrap().rows[0]["label"], "other");
    let (a, b) = tokio::join!(
        db.driver.execute_query(
            Some(&db.name),
            "SELECT current_database()",
            None,
            None,
            None,
            None
        ),
        db.driver.execute_query(
            Some(&other),
            "SELECT current_database()",
            None,
            None,
            None,
            None
        )
    );
    assert_eq!(a.unwrap().rows[0][0], db.name);
    assert_eq!(b.unwrap().rows[0][0], other);
    assert_eq!(
        db.driver.get_tables(&other).await.unwrap()[0].reference,
        target
    );
    assert_eq!(
        db.driver.get_table_structure(&target).await.unwrap()[1].field,
        "label"
    );
    assert_eq!(
        db.driver.get_table_indexes(&target).await.unwrap()[0].column_name,
        "id"
    );
    db.driver
        .apply_table_changes(
            &target,
            vec![RowChange {
                key: vec![change("id", json!(1))],
                changes: vec![change("label", json!("updated"))],
            }],
            vec![],
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        db.driver.get_all_rows(&initial).await.unwrap().1[0]["label"],
        "initial"
    );
    assert_eq!(
        db.driver.get_all_rows(&target).await.unwrap().1[0]["label"],
        "updated"
    );
    let (tx, mut rx) = tokio::sync::mpsc::channel(10);
    db.driver.stream_all_rows(&target, tx).await.unwrap();
    assert_eq!(rx.recv().await.unwrap().1["label"], "updated");
    assert!(db
        .driver
        .drop_tables(&other, &[target.clone(), initial.clone()], false)
        .await
        .is_err());
    db.driver.truncate_table(&target, false).await.unwrap();
    assert!(db.driver.get_all_rows(&target).await.unwrap().1.is_empty());
    assert_eq!(db.driver.get_all_rows(&initial).await.unwrap().1.len(), 1);
    assert!(db.driver.drop_database(&db.name).await.is_err());
    db.driver.drop_database(&other).await.unwrap();
    assert!(!db.driver.get_databases().await.unwrap().contains(&other));
    // Recreating the same name must not retain the old database's pool or tables.
    db.driver.create_database(&other, None, None).await.unwrap();
    assert!(db.driver.get_tables(&other).await.unwrap().is_empty());
    db.driver.drop_database(&other).await.unwrap();
    db.driver.close().await;
    assert!(db.driver.get_tables(&db.name).await.is_err());
    assert!(db.driver.get_tables(&other).await.is_err());
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL; role administration on isolated server"]
async fn optional_database_connection_obeys_server_and_read_only_permissions() {
    use app_lib::connections::{DatabaseSettings, PostgreSqlSettings, PostgreSqlSslMode};
    use app_lib::database::registry::{self, ConnectOptions};
    let db = Database::new().await;
    let role = format!("tupledb_role_{}", Uuid::new_v4().simple());
    let created = format!("tupledb_created_{}", Uuid::new_v4().simple());
    sqlx::query(&format!(
        "CREATE ROLE {role} LOGIN PASSWORD 'tupledb-test-only'"
    ))
    .execute(&db.admin)
    .await
    .unwrap();
    // Only touch permissions on this fixture-owned database, never system databases.
    sqlx::query(&format!(
        "REVOKE CONNECT ON DATABASE {} FROM PUBLIC",
        db.name
    ))
    .execute(&db.admin)
    .await
    .unwrap();
    let base = db.admin.connect_options();
    let mut settings = PostgreSqlSettings {
        host: base.get_host().into(),
        port: base.get_port(),
        user: role.clone(),
        password: Some("tupledb-test-only".into()),
        database: None,
        ssl_mode: PostgreSqlSslMode::Prefer,
        ssl_root_cert: None,
    };
    let options = |read_only| ConnectOptions {
        endpoint: Some((base.get_host(), base.get_port())),
        timeout_secs: 5,
        tunneled: false,
        on_connected: None,
        on_error: None,
        read_only,
    };
    let opened = registry::open(
        &DatabaseSettings::PostgreSql(settings.clone()),
        options(false),
    )
    .await
    .unwrap();
    assert!(!opened.server_version.is_empty());
    assert!(!opened
        .driver
        .get_databases()
        .await
        .unwrap()
        .contains(&db.name));
    assert!(opened.driver.get_tables(&db.name).await.is_err());
    assert!(opened
        .driver
        .create_database(&created, None, None)
        .await
        .is_err());
    sqlx::query(&format!("ALTER ROLE {role} CREATEDB"))
        .execute(&db.admin)
        .await
        .unwrap();
    opened
        .driver
        .create_database(&created, None, None)
        .await
        .unwrap();
    assert!(opened
        .driver
        .get_databases()
        .await
        .unwrap()
        .contains(&created));
    opened
        .driver
        .execute_query(
            Some(&created),
            "CREATE TABLE public.items(id int PRIMARY KEY)",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    let read_only = registry::open(
        &DatabaseSettings::PostgreSql(settings.clone()),
        options(true),
    )
    .await
    .unwrap();
    let result = read_only
        .driver
        .execute_query(
            Some(&created),
            "SHOW default_transaction_read_only",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(result.rows[0][0], "on");
    assert!(read_only
        .driver
        .create_database("must_not_be_created", None, None)
        .await
        .is_err());
    assert!(read_only.driver.drop_database(&created).await.is_err());
    assert!(read_only
        .driver
        .execute_query(
            Some(&created),
            "INSERT INTO public.items VALUES(1)",
            None,
            None,
            None,
            None
        )
        .await
        .is_err());
    read_only.driver.close().await;
    // An explicitly requested inaccessible or missing catalog must not fall back.
    settings.database = Some(db.name.clone());
    assert!(registry::open(
        &DatabaseSettings::PostgreSql(settings.clone()),
        options(false)
    )
    .await
    .is_err());
    settings.database = Some(format!("{created}_missing"));
    assert!(registry::open(
        &DatabaseSettings::PostgreSql(settings.clone()),
        options(false)
    )
    .await
    .is_err());
    opened.driver.drop_database(&created).await.unwrap();
    opened.driver.close().await;
    sqlx::query(&format!("DROP ROLE {role}"))
        .execute(&db.admin)
        .await
        .unwrap();
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn database_creation_applies_encoding_and_native_locale_providers() {
    use sqlx::Row;
    let db = Database::new().await;
    let options = db.driver.get_database_creation_options().await.unwrap();
    assert!(!options.default_character_set.is_empty());
    assert!(!options.default_collation.is_empty());
    assert!(options.collations.iter().any(|c| c.character_set == "UTF8"));
    assert!(options
        .collations
        .iter()
        .any(|c| c.character_set == "LATIN1" && c.name == "\"pg_catalog\".\"C\""));
    assert!(!options.collations.iter().any(|c| c.character_set == "SJIS"));
    let cases = [
        ("libc", "LATIN1", "\"pg_catalog\".\"C\"", "c"),
        ("icu", "UTF8", "\"pg_catalog\".\"und-x-icu\"", "i"),
        ("builtin", "UTF8", "\"pg_catalog\".\"pg_c_utf8\"", "b"),
    ];
    for (suffix, encoding, collation, provider) in cases {
        // Providers depend on server version/build. libc C is always available.
        if !options
            .collations
            .iter()
            .any(|c| c.name == collation && c.character_set == encoding)
        {
            continue;
        }
        let name = format!("{}_{}", db.name, suffix);
        db.driver
            .create_database(&name, Some(encoding), Some(collation))
            .await
            .unwrap();
        let row = sqlx::query(
            "SELECT pg_encoding_to_char(encoding) AS encoding,
            COALESCE(to_jsonb(d)->>'datlocprovider','c') AS provider, datcollate,
            COALESCE(to_jsonb(d)->>'datlocale',to_jsonb(d)->>'daticulocale') AS locale
            FROM pg_database d WHERE datname=$1",
        )
        .bind(&name)
        .fetch_one(&db.admin)
        .await
        .unwrap();
        assert_eq!(row.get::<String, _>("encoding"), encoding);
        assert_eq!(row.get::<String, _>("provider"), provider);
        match provider {
            "c" => assert_eq!(row.get::<String, _>("datcollate"), "C"),
            "i" => assert_eq!(row.get::<String, _>("locale"), "und"),
            "b" => assert_eq!(row.get::<String, _>("locale"), "C.UTF-8"),
            _ => unreachable!(),
        }
        // A non-UTF8 database still connects using the inherited UTF8 client encoding.
        assert!(db.driver.get_tables(&name).await.unwrap().is_empty());
        db.driver.drop_database(&name).await.unwrap();
    }
    let invalid = format!("{}_invalid", db.name);
    assert!(db
        .driver
        .create_database(&invalid, Some("UTF8'; DROP DATABASE postgres; --"), None)
        .await
        .is_err());
    assert!(db
        .driver
        .create_database(&invalid, Some("UTF8"), Some("missing collation"))
        .await
        .is_err());
    if options
        .collations
        .iter()
        .any(|c| c.name == "\"pg_catalog\".\"und-x-icu\"")
    {
        assert!(db
            .driver
            .create_database(
                &invalid,
                Some("LATIN1"),
                Some("\"pg_catalog\".\"und-x-icu\"")
            )
            .await
            .is_err());
    }
    assert!(!db.driver.get_databases().await.unwrap().contains(&invalid));
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn csv_and_json_exports_preserve_schema_identity() {
    use app_lib::services::transfers::{export_database_file, export_table_file, ExportOptions};
    let db = Database::new().await;
    db.seed(
        "CREATE SCHEMA sales; CREATE TABLE public.users(id int); CREATE TABLE sales.users(id int);
        INSERT INTO public.users VALUES(1); INSERT INTO sales.users VALUES(2);
        CREATE TABLE public.empty_table(id int, name text);",
    )
    .await;
    let directory =
        std::env::temp_dir().join(format!("tupledb_pg_export_{}", Uuid::new_v4().simple()));
    std::fs::create_dir(&directory).unwrap();
    for format in ["json", "csv"] {
        let path = directory.join(format!("backup.{format}"));
        let count = export_database_file(
            db.driver.clone(),
            db.name.clone(),
            "data".into(),
            path.to_str().unwrap().into(),
            None,
            format,
            ExportOptions {
                drop_if_exists: false,
                include_views: true,
                use_transactions: false,
                compress_gzip: false,
            },
            &|_| {},
            &|| false,
        )
        .await
        .unwrap();
        assert_eq!(count, 2);
    }
    let json: Value =
        serde_json::from_str(&std::fs::read_to_string(directory.join("backup.json")).unwrap())
            .unwrap();
    assert_eq!(json["[\"public\",\"users\"]"][0]["id"], 1);
    assert_eq!(json["[\"sales\",\"users\"]"][0]["id"], 2);
    assert!(
        std::fs::read_to_string(directory.join("backup_public--users.csv"))
            .unwrap()
            .contains("1")
    );
    assert!(
        std::fs::read_to_string(directory.join("backup_sales--users.csv"))
            .unwrap()
            .contains("2")
    );
    assert_eq!(
        std::fs::read_to_string(directory.join("backup_public--empty_table.csv")).unwrap(),
        "id,name\n"
    );
    assert_eq!(json["[\"public\",\"empty_table\"]"], json!([]));
    for format in ["csv", "json"] {
        let path = directory.join(format!("empty.{format}"));
        let count = export_table_file(
            db.driver.clone(),
            db.name.clone(),
            db.table("public", "empty_table"),
            format.into(),
            path.to_str().unwrap().into(),
            &|_, _, _| {},
        )
        .await
        .unwrap();
        assert_eq!(count, 0);
        assert_eq!(
            std::fs::read_to_string(path).unwrap(),
            if format == "csv" { "id,name\n" } else { "[]" }
        );
    }
    std::fs::remove_dir_all(&directory).unwrap();
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn schemas_types_filters_and_pagination_preserve_identity() {
    let db = Database::new().await;
    db.seed("CREATE SCHEMA sales; CREATE TABLE public.users(id bigint PRIMARY KEY, name text, active boolean, amount numeric(65,30), meta jsonb, tags text[]);
        CREATE TABLE sales.users(id int PRIMARY KEY,name text);
        INSERT INTO sales.users VALUES(1,'other schema');
        INSERT INTO public.users VALUES(1,'Ada',true,12345678901234567890123456789012345.123456789012345678901234567890,'{\"id\":9223372036854775807}',ARRAY['a','b']),
        (2,'Grace',false,NULL,NULL,NULL),(9223372036854775807,'Large',true,NULL,NULL,NULL);").await;
    let tables = db.driver.get_tables(&db.name).await.unwrap();
    assert_eq!(tables.iter().filter(|t| t.name == "users").count(), 2);
    assert!(tables
        .iter()
        .any(|t| t.reference == db.table("sales", "users")));
    let target = db.table("public", "users");
    let result = db
        .driver
        .get_table_data(&target, 0, 10, None, None, None, true, None)
        .await
        .unwrap();
    assert_eq!(result.total_count, 3);
    assert_eq!(result.rows[0]["active"], true);
    assert_eq!(
        result.rows[0]["amount"],
        "12345678901234567890123456789012345.123456789012345678901234567890"
    );
    assert!(result.rows[0]["meta"]
        .as_str()
        .unwrap()
        .contains("9223372036854775807"));
    assert_eq!(result.rows[2]["id"], "9223372036854775807");
    let filter = FilterSet {
        match_all: true,
        rows: vec![FilterRow {
            active: true,
            column: "name".into(),
            operator: Operator::Contains,
            value: "a".into(),
        }],
    };
    let filtered = db
        .driver
        .get_table_data(
            &target,
            0,
            10,
            Some(filter),
            Some("name".into()),
            Some(true),
            true,
            None,
        )
        .await
        .unwrap();
    assert_eq!(filtered.total_count, 3);
    assert_eq!(filtered.rows[0]["name"], "Large");
    let next = db
        .driver
        .get_table_data(
            &target,
            1,
            1,
            None,
            None,
            None,
            true,
            Some(KeysetPage {
                column: "id".into(),
                value: json!(1),
                direction: "next".into(),
            }),
        )
        .await
        .unwrap();
    assert_eq!(next.rows[0]["id"], 2);
    let empty = db
        .driver
        .get_table_data(&target, 99, 10, None, None, None, true, None)
        .await
        .unwrap();
    assert_eq!(empty.columns.len(), 6);
    assert!(empty.rows.is_empty());
    assert!(db
        .driver
        .get_all_rows(&TableRef::new(&db.name, "users"))
        .await
        .is_err());
    assert!(db.driver.get_tables("another_database").await.is_err());
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn composite_edits_are_atomic_and_constraints_are_preserved() {
    let db = Database::new().await;
    db.seed("CREATE SCHEMA sales;
        CREATE TABLE sales.items(id int,tenant int,note text,derived int GENERATED ALWAYS AS(id+1) STORED,PRIMARY KEY(tenant,id));
        INSERT INTO sales.items(id,tenant,note) VALUES(1,1,'first'),(2,1,'second'),(1,2,'other');
        CREATE TABLE public.related(tenant int,id int,CONSTRAINT item_ref FOREIGN KEY(tenant,id) REFERENCES sales.items(tenant,id));
        CREATE INDEX note_idx ON sales.items(note) WHERE note IS NOT NULL;").await;
    let table = db.table("sales", "items");
    let columns = db.driver.get_table_structure(&table).await.unwrap();
    assert_eq!(columns[0].primary_key_position, Some(2));
    assert_eq!(columns[1].primary_key_position, Some(1));
    assert!(columns[3].is_generated);
    let fks = db
        .driver
        .get_foreign_keys(&db.table("public", "related"))
        .await
        .unwrap();
    assert_eq!(fks.len(), 2);
    assert_eq!(fks[1].position, 2);
    assert_eq!(fks[0].referenced, table);
    assert!(db
        .driver
        .get_table_indexes(&table)
        .await
        .unwrap()
        .iter()
        .any(|i| i.key_name == "note_idx"));
    assert!(db
        .driver
        .apply_table_changes(
            &table,
            vec![],
            vec![RowDeletion {
                key: vec![change("tenant", json!(1))]
            }],
            false
        )
        .await
        .is_err());
    db.driver
        .apply_table_changes(
            &table,
            vec![RowChange {
                key: vec![change("tenant", json!(1)), change("id", json!(1))],
                changes: vec![change("id", json!(3)), change("note", json!("changed"))],
            }],
            vec![RowDeletion {
                key: vec![change("tenant", json!(1)), change("id", json!(2))],
            }],
            false,
        )
        .await
        .unwrap();
    let rows = db.driver.get_all_rows(&table).await.unwrap().1;
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().any(|r| r["id"] == 3 && r["note"] == "changed"));
    assert!(rows
        .iter()
        .any(|r| r["tenant"] == 2 && r["note"] == "other"));
    // A later invalid deletion must roll back the earlier valid update.
    assert!(db
        .driver
        .apply_table_changes(
            &table,
            vec![RowChange {
                key: vec![change("tenant", json!(1)), change("id", json!(3))],
                changes: vec![change("note", json!("rollback"))]
            }],
            vec![RowDeletion {
                key: vec![change("tenant", json!(999)), change("id", json!(999))]
            }],
            false
        )
        .await
        .is_err());
    assert!(!db
        .driver
        .get_all_rows(&table)
        .await
        .unwrap()
        .1
        .iter()
        .any(|r| r["note"] == "rollback"));
    db.seed("INSERT INTO public.related VALUES(1,3)").await;
    assert!(db.driver.drop_table(&table, false).await.is_err());
    assert!(db.driver.truncate_table(&table, true).await.is_err());
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn inserts_preserve_literals_defaults_and_quoted_names() {
    let db = Database::new().await;
    db.seed("CREATE SCHEMA \"odd.schema\"; CREATE TABLE \"odd.schema\".\"a\"\"b\"(id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY, note text DEFAULT 'default', amount numeric, active boolean)").await;
    let table = db.table("odd.schema", "a\"b");
    db.driver.insert_row(&table, vec![], false).await.unwrap();
    db.driver
        .insert_row(
            &table,
            vec![
                change("note", json!("NOW(); DROP TABLE public.users;")),
                change("amount", json!("12345678901234567890.123456789")),
                change("active", json!(false)),
            ],
            false,
        )
        .await
        .unwrap();
    let rows = db.driver.get_all_rows(&table).await.unwrap().1;
    assert_eq!(rows[0]["note"], "default");
    assert_eq!(rows[1]["note"], "NOW(); DROP TABLE public.users;");
    assert_eq!(rows[1]["amount"], "12345678901234567890.123456789");
    assert_eq!(rows[1]["active"], false);
    db.driver.truncate_table(&table, false).await.unwrap();
    assert!(db.driver.get_all_rows(&table).await.unwrap().1.is_empty());
    db.driver.drop_table(&table, false).await.unwrap();
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn identity_columns_only_block_generated_always() {
    let db = Database::new().await;
    db.seed(
        "CREATE TABLE public.gen_ids (
            id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
            optional_id integer GENERATED BY DEFAULT AS IDENTITY,
            serial_id serial,
            note text
        )",
    )
    .await;
    let table = db.table("public", "gen_ids");
    let structure = db.driver.get_table_structure(&table).await.unwrap();
    let is_identity = |name: &str| {
        structure
            .iter()
            .find(|c| c.field == name)
            .unwrap()
            .is_identity
    };
    assert!(is_identity("id"), "GENERATED ALWAYS stays non-editable");
    assert!(
        !is_identity("optional_id"),
        "GENERATED BY DEFAULT accepts explicit values"
    );
    assert!(!is_identity("serial_id"), "SERIAL accepts explicit values");

    // GENERATED ALWAYS still rejects an explicit value.
    assert!(db
        .driver
        .insert_row(&table, vec![change("id", json!(999))], false)
        .await
        .is_err());

    // BY DEFAULT and SERIAL accept an explicit value...
    db.driver
        .insert_row(
            &table,
            vec![
                change("optional_id", json!(500)),
                change("serial_id", json!(500)),
                change("note", json!("explicit")),
            ],
            false,
        )
        .await
        .unwrap();
    // ...and can still be left out to fall back to the sequence default.
    db.driver
        .insert_row(&table, vec![change("note", json!("defaulted"))], false)
        .await
        .unwrap();
    let rows = db.driver.get_all_rows(&table).await.unwrap().1;
    let explicit = rows.iter().find(|r| r["note"] == "explicit").unwrap();
    assert_eq!(explicit["optional_id"], 500);
    assert_eq!(explicit["serial_id"], 500);
    assert!(rows.iter().any(|r| r["note"] == "defaulted"));
    db.driver.drop_table(&table, false).await.unwrap();
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn query_streaming_keeps_metadata_limits_and_duplicate_column_names() {
    let db = Database::new().await;
    let chunks = Arc::new(Mutex::new(Vec::new()));
    let captured = chunks.clone();
    let result = db
        .driver
        .execute_query(
            Some(&db.name),
            "SELECT n AS value, n+1 AS value FROM generate_series(1,2000) n",
            None,
            None,
            Some(Arc::new(move |_, rows| {
                captured.lock().unwrap().extend(rows)
            })),
            Some(20),
        )
        .await
        .unwrap();
    assert!(result.is_select);
    assert_eq!(result.rows_affected, 2000);
    assert_eq!(result.columns.len(), 2);
    let rows = chunks.lock().unwrap();
    assert_eq!(rows.len(), 10);
    assert_eq!(rows[0], json!([1, 2]));
    drop(rows);
    let empty = db
        .driver
        .execute_query(
            None,
            "SELECT 1::int AS id WHERE false",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert!(empty.is_select);
    assert!(empty.rows.is_empty());
    assert_eq!(empty.columns[0].name, "id");
    db.seed("CREATE TABLE public.edits(id int)").await;
    let returning = db
        .driver
        .execute_query(
            None,
            "INSERT INTO public.edits VALUES(1) RETURNING id",
            None,
            None,
            None,
            None,
        )
        .await
        .unwrap();
    assert_eq!(returning.rows, json!([[1]]).as_array().unwrap().clone());
    assert!(db
        .driver
        .execute_query(
            None,
            "SELECT 1; DELETE FROM public.edits",
            None,
            None,
            None,
            None
        )
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM public.edits")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        1
    );
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn read_only_queries_cannot_write_through_functions() {
    let db = Database::new().await;
    db.seed("CREATE TABLE public.edits(id int);
        CREATE FUNCTION public.write_row() RETURNS int LANGUAGE plpgsql AS $$ BEGIN INSERT INTO public.edits VALUES(1); RETURN 1; END $$;").await;
    let driver = PostgreSqlDriver::new(db.pool.clone(), db.name.clone(), true);
    assert!(driver
        .execute_query(None, "SELECT public.write_row()", None, None, None, None)
        .await
        .is_err());
    assert!(driver
        .execute_query(
            None,
            "EXPLAIN ANALYZE INSERT INTO public.edits VALUES(1)",
            None,
            None,
            None,
            None
        )
        .await
        .is_err());
    assert!(driver
        .insert_row(
            &db.table("public", "edits"),
            vec![change("id", json!(1))],
            false
        )
        .await
        .is_err());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM public.edits")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        0
    );
    assert!(driver.capabilities().import_sql);
    assert!(driver.capabilities().export_sql);
    assert!(driver.capabilities().alter_columns);
    assert!(driver.capabilities().inspect_ddl);
    assert!(driver.capabilities().cancel_query);
    db.close().await;
}
