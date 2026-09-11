use super::*;
use std::time::Duration;

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn regression_bound_values_do_not_truncate_or_change_filter_values() {
    let db = Database::new().await;
    db.seed("CREATE DOMAIN public.short_word AS varchar(3) CHECK(VALUE <> 'bad');
        CREATE DOMAIN public.nested_word AS public.short_word;
        CREATE DOMAIN public.word_list AS varchar(3)[];
        CREATE TABLE public.lengths(id integer PRIMARY KEY, v varchar(3), c char(3), d public.nested_word, a varchar(3)[], da public.short_word[], nested public.word_list[], bits bit(3), amount numeric(4,1));
        INSERT INTO public.lengths(id,v,c,d,a,da,bits,amount) VALUES(1,'abc','abc','abc',ARRAY['abc'],ARRAY['abc']::public.short_word[], B'101',1.2)").await;
    let table = db.table("public", "lengths");
    for (name, value) in [
        ("v", "abcdef"),
        ("c", "abcdef"),
        ("d", "abcdef"),
        ("d", "bad"),
        ("a", "{abcdef}"),
        ("da", "{abcdef}"),
        ("da", "{bad}"),
        ("nested", r#"{"{abcdef}"}"#),
        ("bits", "101011"),
    ] {
        assert!(
            db.driver
                .insert_row(
                    &table,
                    vec![change("id", json!(2)), change(name, json!(value))],
                    false
                )
                .await
                .is_err(),
            "insert {name}={value}"
        );
        assert!(
            db.driver
                .apply_table_changes(
                    &table,
                    vec![RowChange {
                        key: vec![change("id", json!(1))],
                        changes: vec![change(name, json!(value))]
                    }],
                    vec![],
                    false
                )
                .await
                .is_err(),
            "update {name}={value}"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT v FROM public.lengths WHERE id=1")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        "abc"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM public.lengths")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        1
    );
    for (name, value) in [("v", "abcdef"), ("c", "abcdef"), ("amount", "1.23")] {
        let rows = db
            .driver
            .get_table_data(
                &table,
                0,
                10,
                Some(FilterSet {
                    match_all: true,
                    rows: vec![FilterRow {
                        active: true,
                        column: name.into(),
                        operator: Operator::Equals,
                        value: value.into(),
                    }],
                }),
                None,
                None,
                true,
                None,
            )
            .await
            .unwrap();
        assert!(
            rows.rows.is_empty(),
            "filter {name}={value} must not be narrowed before comparison"
        );
    }
    db.driver
        .insert_row(
            &table,
            vec![
                change("id", json!(2)),
                change("v", json!("á🐘好")),
                change("c", json!("a")),
                change("d", json!("ok")),
                change("a", json!("{a,bc}")),
                change("da", json!("{a,bc}")),
                change("nested", json!(r#"{"{a,bc}","{xyz}"}"#)),
            ],
            false,
        )
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT v FROM public.lengths WHERE id=2")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        "á🐘好"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT nested::text FROM public.lengths WHERE id=2")
            .fetch_one(&db.pool)
            .await
            .unwrap(),
        r#"{"{a,bc}","{xyz}"}"#
    );
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn regression_sort_and_keyset_use_native_values_not_text_projection() {
    let db = Database::new().await;
    db.seed("CREATE SCHEMA \"odd schema\"; CREATE TABLE \"odd schema\".\"odd table\"(\"odd id\" int PRIMARY KEY, amount numeric); INSERT INTO \"odd schema\".\"odd table\" VALUES(1,2),(2,10),(10,-1),(11,2)").await;
    let table = db.table("odd schema", "odd table");
    let first = db
        .driver
        .get_table_data(&table, 0, 2, None, None, None, true, None)
        .await
        .unwrap();
    assert_eq!(
        first
            .rows
            .iter()
            .map(|r| r["odd id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    let next = db
        .driver
        .get_table_data(
            &table,
            1,
            2,
            None,
            None,
            None,
            true,
            Some(KeysetPage {
                column: "odd id".into(),
                value: first.rows[1]["odd id"].clone(),
                direction: "next".into(),
            }),
        )
        .await
        .unwrap();
    assert_eq!(
        next.rows
            .iter()
            .map(|r| r["odd id"].as_i64().unwrap())
            .collect::<Vec<_>>(),
        vec![10, 11]
    );
    let prev = db
        .driver
        .get_table_data(
            &table,
            0,
            2,
            None,
            None,
            None,
            true,
            Some(KeysetPage {
                column: "odd id".into(),
                value: next.rows[0]["odd id"].clone(),
                direction: "prev".into(),
            }),
        )
        .await
        .unwrap();
    assert_eq!(prev.rows, first.rows);
    let offset = db
        .driver
        .get_table_data(&table, 1, 2, None, None, None, true, None)
        .await
        .unwrap();
    assert_eq!(offset.rows, next.rows);
    for (desc, expected) in [(false, vec![10, 1, 11, 2]), (true, vec![2, 1, 11, 10])] {
        let sorted = db
            .driver
            .get_table_data(
                &table,
                0,
                10,
                None,
                Some("amount".into()),
                Some(desc),
                true,
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            sorted
                .rows
                .iter()
                .map(|r| r["odd id"].as_i64().unwrap())
                .collect::<Vec<_>>(),
            expected
        );
    }
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn regression_primary_key_include_is_not_part_of_row_identity() {
    let db = Database::new().await;
    db.seed("CREATE TABLE public.covering(id int, note text, PRIMARY KEY(id) INCLUDE(note)); INSERT INTO public.covering VALUES(1,NULL),(2,'two'); CREATE TABLE public.compound(a int,b int,note text, PRIMARY KEY(b,a) INCLUDE(note)); INSERT INTO public.compound VALUES(1,2,NULL)").await;
    let table = db.table("public", "covering");
    assert_eq!(
        db.driver.get_primary_key_columns(&table).await.unwrap(),
        vec!["id"]
    );
    let structure = db.driver.get_table_structure(&table).await.unwrap();
    assert!(structure[1].primary_key_position.is_none());
    assert!(structure[1].nullable);
    db.driver
        .apply_table_changes(
            &table,
            vec![RowChange {
                key: vec![change("id", json!(1))],
                changes: vec![change("note", json!("updated"))],
            }],
            vec![],
            false,
        )
        .await
        .unwrap();
    db.driver
        .apply_table_changes(
            &table,
            vec![],
            vec![RowDeletion {
                key: vec![change("id", json!(1))],
            }],
            false,
        )
        .await
        .unwrap();
    let page = db
        .driver
        .get_table_data(
            &table,
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
    assert_eq!(page.rows[0]["id"], 2);
    let compound = db.table("public", "compound");
    assert_eq!(
        db.driver.get_primary_key_columns(&compound).await.unwrap(),
        vec!["b", "a"]
    );
    db.driver
        .apply_table_changes(
            &compound,
            vec![],
            vec![RowDeletion {
                key: vec![change("b", json!(2)), change("a", json!(1))],
            }],
            false,
        )
        .await
        .unwrap();
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn regression_pg_prefix_schemas_remain_visible() {
    let db = Database::new().await;
    db.seed("CREATE SCHEMA pgapp; CREATE TABLE pgapp.visible(id int); CREATE SCHEMA \"pg-app\"; CREATE TABLE \"pg-app\".visible(id int); CREATE SCHEMA pg; CREATE TABLE pg.visible(id int)").await;
    let tables = db.driver.get_tables(&db.name).await.unwrap();
    for schema in ["pgapp", "pg-app", "pg"] {
        assert!(
            tables
                .iter()
                .any(|t| t.reference == db.table(schema, "visible")),
            "{schema}"
        );
    }
    assert!(!tables
        .iter()
        .any(|t| t.reference.schema.as_deref().unwrap().starts_with("pg_")));
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn column_changes_preserve_metadata_and_rollback_together() {
    let db = Database::new().await;
    db.seed("CREATE TABLE public.edits(id integer GENERATED ALWAYS AS IDENTITY PRIMARY KEY, label varchar(20) COLLATE \"C\" NOT NULL DEFAULT 'test', value integer CHECK(value>0)); COMMENT ON COLUMN public.edits.label IS 'keep me'; INSERT INTO public.edits(label,value) VALUES ('hello',12)").await;
    let table = db.table("public", "edits");
    let sql = db
        .driver
        .alter_table_column(&table, "label", "new label", "varchar(80)")
        .await
        .unwrap();
    assert!(sql.contains("RENAME COLUMN"));
    let metadata: (String, bool, String, String) = sqlx::query_as("SELECT format_type(atttypid,atttypmod),attnotnull,attcollation::regcollation::text,col_description(attrelid,attnum) FROM pg_attribute WHERE attrelid='public.edits'::regclass AND attname='new label'").fetch_one(&db.pool).await.unwrap();
    assert_eq!(
        metadata,
        (
            "character varying(80)".into(),
            true,
            "\"C\"".into(),
            "keep me".into()
        )
    );
    let default: String =
        sqlx::query_scalar("INSERT INTO public.edits(value) VALUES(1) RETURNING \"new label\"")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(default, "test");
    db.driver
        .alter_table_column(&table, "id", "new id", "bigint")
        .await
        .unwrap();
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "INSERT INTO public.edits(value) VALUES(2) RETURNING \"new id\""
        )
        .fetch_one(&db.pool)
        .await
        .unwrap(),
        3
    );
    // Type conversion succeeds but the rename fails: both must roll back.
    assert!(db
        .driver
        .alter_table_column(&table, "value", "new label", "bigint")
        .await
        .is_err());
    assert_eq!(sqlx::query_scalar::<_,String>("SELECT format_type(atttypid,atttypmod) FROM pg_attribute WHERE attrelid='public.edits'::regclass AND attname='value'").fetch_one(&db.pool).await.unwrap(),"integer");
    assert!(db
        .driver
        .alter_table_column(&table, "new label", "converted", "integer")
        .await
        .is_err());
    assert!(db
        .driver
        .alter_table_column(
            &table,
            "value",
            "value",
            "integer, DROP COLUMN \"new label\""
        )
        .await
        .is_err());
    assert!(db
        .driver
        .alter_table_column(&table, "value", "value", "integer; DROP TABLE public.edits")
        .await
        .is_err());
    let ro = PostgreSqlDriver::new(db.pool.clone(), db.name.clone(), true);
    assert!(ro
        .alter_table_column(&table, "value", "forbidden", "integer")
        .await
        .is_err());
    assert!(sqlx::query("INSERT INTO public.edits(value) VALUES(-1)")
        .execute(&db.pool)
        .await
        .is_err());
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn column_types_support_domains_arrays_and_qualified_identifiers() {
    let db = Database::new().await;
    db.seed("CREATE SCHEMA \"Types\"; CREATE DOMAIN \"Types\".\"Amount\" AS numeric(12,2) CHECK(VALUE>=0); CREATE TABLE public.typed(amount numeric(10,2), times timestamp[], duration interval, bits bit(2)); INSERT INTO public.typed VALUES(12.34, ARRAY['2026-01-01'::timestamp], '1 day', B'01')").await;
    let table = db.table("public", "typed");
    for (name, ty) in [
        ("amount", "\"Types\".\"Amount\""),
        ("times", "timestamp(3) with time zone[]"),
        ("duration", "interval day to second(2)"),
        ("bits", "bit varying(8)"),
    ] {
        db.driver
            .alter_table_column(&table, name, name, ty)
            .await
            .unwrap();
    }
    assert!(sqlx::query("UPDATE public.typed SET amount=-1")
        .execute(&db.pool)
        .await
        .is_err());
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn inspected_ddl_recreates_table_sequence_constraints_indexes_and_comments() {
    let source = Database::new().await;
    source.seed("CREATE TABLE public.ddl(id serial PRIMARY KEY, name text NOT NULL DEFAULT 'hello', generated text GENERATED ALWAYS AS (upper(name)) STORED, CHECK(length(name)>0)); CREATE INDEX ddl_name ON public.ddl(lower(name)) WHERE id>0; COMMENT ON TABLE public.ddl IS 'table comment'; COMMENT ON COLUMN public.ddl.name IS 'name comment'; CREATE VIEW public.ddl_view AS SELECT id,name FROM public.ddl").await;
    let ddl = source
        .driver
        .get_table_ddl(&source.table("public", "ddl"))
        .await
        .unwrap();
    assert!(ddl.contains("CREATE SEQUENCE"));
    assert!(ddl.contains("PRIMARY KEY"));
    assert!(ddl.contains("CREATE INDEX"));
    assert!(ddl.contains("name comment"));
    assert!(!ddl.contains("CREATE ROLE"));
    let target = Database::new().await;
    target.seed(&ddl).await;
    let row: (i32, String) =
        sqlx::query_as("INSERT INTO public.ddl DEFAULT VALUES RETURNING id,generated")
            .fetch_one(&target.pool)
            .await
            .unwrap();
    assert_eq!(row, (1, "HELLO".into()));
    let view = source
        .driver
        .get_table_ddl(&source.table("public", "ddl_view"))
        .await
        .unwrap();
    target.seed(&view).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM public.ddl_view")
            .fetch_one(&target.pool)
            .await
            .unwrap(),
        1
    );
    source.close().await;
    target.close().await;
}

async fn wait_for_query(db: &Database, tag: &str) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let active: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=$1 AND query LIKE $2 AND state='active' AND pid<>pg_backend_pid())")
                .bind(&db.name).bind(format!("%{tag}%")).fetch_one(&db.admin).await.unwrap();
            if active { break; }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }).await.unwrap();
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn cancel_query_targets_one_session_and_releases_single_slot_pool() {
    let db = Database::new().await;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(db.pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    let driver = Arc::new(PostgreSqlDriver::new(pool, db.name.clone(), false));
    let worker_driver = driver.clone();
    let worker = tokio::spawn(async move {
        worker_driver
            .execute_query(
                None,
                "SELECT pg_sleep(30) /*cancel_one_slot*/",
                Some("slow"),
                None,
                None,
                None,
            )
            .await
    });
    wait_for_query(&db, "cancel_one_slot").await;
    driver.cancel_query("unknown").await.unwrap();
    driver.cancel_query("slow").await.unwrap();
    let error = tokio::time::timeout(Duration::from_secs(8), worker)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err();
    assert!(error.contains("cancelled"), "{error}");
    // The registration is removed; an ID can be reused without stale cancellation.
    let result = driver
        .execute_query(None, "SELECT 42 AS answer", Some("slow"), None, None, None)
        .await
        .unwrap();
    assert_eq!(result.rows[0][0], 42);
    driver.cancel_query("slow").await.unwrap();
    driver.close().await;
    db.close().await;
}

#[tokio::test]
#[ignore = "requires TUPLEDB_TEST_POSTGRESQL_URL"]
async fn cancel_queued_query_does_not_cancel_running_query_and_disconnect_stops_it() {
    let db = Database::new().await;
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect_with(db.pool.connect_options().as_ref().clone())
        .await
        .unwrap();
    let driver = Arc::new(PostgreSqlDriver::new(pool, db.name.clone(), false));
    let first_driver = driver.clone();
    let first = tokio::spawn(async move {
        first_driver
            .execute_query(
                None,
                "SELECT pg_sleep(30) /*still_running*/",
                Some("first"),
                None,
                None,
                None,
            )
            .await
    });
    wait_for_query(&db, "still_running").await;
    let second_driver = driver.clone();
    let second = tokio::spawn(async move {
        second_driver
            .execute_query(None, "SELECT 1", Some("queued"), None, None, None)
            .await
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    driver.cancel_query("queued").await.unwrap();
    assert!(tokio::time::timeout(Duration::from_secs(2), second)
        .await
        .unwrap()
        .unwrap()
        .unwrap_err()
        .contains("cancelled"));
    assert!(!first.is_finished());
    tokio::time::timeout(Duration::from_secs(8), driver.close())
        .await
        .unwrap();
    assert!(first.await.unwrap().unwrap_err().contains("cancelled"));
    db.close().await;
}
