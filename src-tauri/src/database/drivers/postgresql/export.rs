//! Dependency-ordered native SQL export. Values travel through PostgreSQL COPY,
//! never through JSON/JavaScript or the display-oriented value decoder.
use super::{sql, PostgreSqlDriver};
use crate::database::driver::*;
use futures::TryStreamExt;
use sqlx::{Executor, PgConnection, Row};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    time::Duration,
};

type Key = (String, i64);
fn key(class: &str, oid: i64) -> Key {
    (class.into(), oid)
}
fn ident(name: &str) -> Result<String, String> {
    sql::quote_identifier(name)
}
fn literal(text: &str) -> String {
    format!("E'{}'", text.replace('\\', "\\\\").replace('\'', "''"))
}
fn name(schema: &str, object: &str) -> Result<String, String> {
    Ok(format!("{}.{}", ident(schema)?, ident(object)?))
}
fn write(writer: &mut (dyn Write + Send), text: &str) -> Result<(), String> {
    writer
        .write_all(text.as_bytes())
        .map_err(|e| format!("Cannot write SQL export: {e}"))
}

struct Node {
    schema: String,
    label: String,
    create: String,
    drop: String,
    deps: BTreeSet<Key>,
    post: bool,
    owner: Option<i64>,
    error: Option<String>,
}
impl Node {
    fn new(schema: &str, label: &str, create: String, drop: String) -> Self {
        Self {
            schema: schema.into(),
            label: label.into(),
            create,
            drop,
            deps: BTreeSet::new(),
            post: false,
            owner: None,
            error: None,
        }
    }
}
struct Relation {
    reference: TableRef,
    kind: String,
    columns: Vec<String>,
    populated: bool,
    parent: Option<i64>,
}
struct Sequence {
    schema: String,
    oid: i64,
    qualified: String,
    owner: Option<(i64, String)>,
    identity: bool,
    options: String,
}
struct Plan {
    nodes: BTreeMap<Key, Node>,
    aliases: BTreeMap<Key, Key>,
    relations: BTreeMap<i64, Relation>,
    sequences: Vec<Sequence>,
    schema_comments: BTreeMap<String, Option<String>>,
    comments: Vec<(Key, String)>,
}

impl PostgreSqlDriver {
    pub(super) async fn inspect_table_ddl(&self, table: &TableRef) -> Result<String, String> {
        let qualified = self.relation(table)?;
        let pool = self.pool_for(&table.catalog).await?;
        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
        (&mut *tx).execute("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL search_path=pg_catalog").await.map_err(|e| e.to_string())?;
        let plan = Plan::load(&mut tx, &table.catalog).await?;
        let oid = plan
            .relations
            .iter()
            .find(|(_, r)| &r.reference == table)
            .map(|(oid, _)| *oid)
            .ok_or("Table or view no longer exists")?;
        let mut needed = BTreeSet::from([key("pg_class", oid)]);
        needed.extend(
            plan.nodes
                .iter()
                .filter(|(_, n)| n.owner == Some(oid))
                .map(|(k, _)| k.clone()),
        );
        needed.extend(
            plan.sequences
                .iter()
                .filter(|s| !s.identity && s.owner.as_ref().is_some_and(|(owner, _)| *owner == oid))
                .map(|s| key("pg_class", s.oid)),
        );
        let label = qualified.replace(['\r', '\n'], " ");
        let mut ddl = format!("-- Definition of {label}. Referenced schemas, types, functions and other tables must already exist.\n");
        let mut remaining = needed.clone();
        while !remaining.is_empty() {
            let next = remaining
                .iter()
                .find(|k| {
                    plan.nodes[*k]
                        .deps
                        .iter()
                        .all(|dep| !remaining.contains(dep))
                })
                .cloned()
                .ok_or("Circular DDL dependencies")?;
            let node = &plan.nodes[&next];
            if let Some(error) = &node.error {
                return Err(format!("Cannot inspect {}: {error}", node.label));
            }
            ddl.push_str(node.create.trim_end_matches(';'));
            ddl.push_str(";\n");
            remaining.remove(&next);
        }
        for seq in plan.sequences.iter().filter(|s| !s.identity) {
            if let Some((owner, column)) = &seq.owner {
                if *owner == oid {
                    ddl.push_str(&format!(
                        "ALTER SEQUENCE {} OWNED BY {qualified}.{};\n",
                        seq.qualified,
                        ident(column)?
                    ));
                }
            }
        }
        for (owner, comment) in &plan.comments {
            if needed.contains(owner) {
                ddl.push_str(comment);
                ddl.push_str(";\n");
            }
        }
        tx.rollback().await.map_err(|e| e.to_string())?;
        Ok(ddl)
    }

    pub(super) async fn export_native(
        &self,
        database: &str,
        tables: &[TableRef],
        options: &SqlExportOptions,
        writer: &mut (dyn Write + Send),
        is_canceled: &(dyn Fn() -> bool + Send + Sync),
        progress: &(dyn Fn(usize, usize, String) + Send + Sync),
    ) -> Result<usize, String> {
        if !["full", "structure", "data"].contains(&options.mode.as_str()) {
            return Err("Unknown SQL export mode".into());
        }
        let pool = self.pool_for(database).await?;
        let mut conn = pool.acquire().await.map_err(|e| e.to_string())?;
        conn.close_on_drop();
        let (pid, started): (i32, String) = sqlx::query_as("SELECT pg_backend_pid(), backend_start::text FROM pg_stat_activity WHERE pid=pg_backend_pid()")
            .fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
        let result = tokio::select! {
            biased;
            _ = async {
                loop {
                    if is_canceled() || pool.is_closed() { break; }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
            } => {
                super::session::cancel_backend(&pool, database, pid, &started).await;
                Err("Export cancelled".into())
            },
            result = export_session(&mut conn, database, tables, options, writer, progress) => result,
        };
        // This session is disposable even when COPY was interrupted mid-protocol.
        let _ = tokio::time::timeout(Duration::from_secs(5), conn.close()).await;
        result
    }
}

async fn export_session(
    conn: &mut PgConnection,
    database: &str,
    tables: &[TableRef],
    options: &SqlExportOptions,
    writer: &mut (dyn Write + Send),
    progress: &(dyn Fn(usize, usize, String) + Send + Sync),
) -> Result<usize, String> {
    (&mut *conn).execute("BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; SET LOCAL search_path=pg_catalog; SET LOCAL standard_conforming_strings=on; SET LOCAL client_encoding='UTF8'; SET LOCAL DateStyle='ISO, YMD'; SET LOCAL IntervalStyle='postgres'; SET LOCAL TimeZone='UTC'; SET LOCAL extra_float_digits=3; SET LOCAL bytea_output='hex'; SET LOCAL row_security=off; SET LOCAL lock_timeout='10s'")
        .await.map_err(|e| e.to_string())?;
    progress(
        0,
        tables.len(),
        "Reading PostgreSQL schema and dependencies…".into(),
    );
    // Lock before reading column definitions/defaults. Locking a partitioned
    // parent also locks its descendants. Take a deterministic order.
    let mut lock_targets = tables
        .iter()
        .map(sql::relation)
        .collect::<Result<Vec<_>, _>>()?;
    lock_targets.sort();
    lock_targets.dedup();
    for qualified in lock_targets {
        let kind: Option<String> =
            sqlx::query_scalar("SELECT relkind::text FROM pg_class WHERE oid=to_regclass($1)")
                .bind(&qualified)
                .fetch_optional(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
        if matches!(kind.as_deref(), Some("r" | "p")) {
            (&mut *conn)
                .execute(format!("LOCK TABLE {qualified} IN ACCESS SHARE MODE").as_str())
                .await
                .map_err(|e| format!("Cannot lock {qualified} for export: {e}"))?;
        }
    }
    let plan = Plan::load(conn, database).await?;
    let has_large_objects: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_largeobject_metadata)")
            .fetch_one(&mut *conn)
            .await
            .map_err(|e| e.to_string())?;
    if has_large_objects {
        return Err("This database contains PostgreSQL large objects. Use pg_dump to include their contents; native SQL export does not copy large objects".into());
    }
    let mut selected = tables
        .iter()
        .map(|t| {
            plan.relations
                .iter()
                .find(|(_, r)| r.reference == *t)
                .map(|(oid, _)| *oid)
                .ok_or_else(|| format!("Unknown export table {t}"))
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    loop {
        let children: Vec<_> = plan
            .relations
            .iter()
            .filter(|(_, r)| r.parent.is_some_and(|p| selected.contains(&p)))
            .map(|(id, _)| *id)
            .collect();
        let before = selected.len();
        selected.extend(children);
        if selected.len() == before {
            break;
        }
    }
    if selected.is_empty() {
        return Err("Select at least one table or view to export".into());
    }
    let structure = options.mode != "data";
    let data = options.mode != "structure";
    let order = plan.order(&selected, structure)?;
    // Money's text representation depends on locale; C is available on every
    // server and makes COPY round trips independent of the source OS locale.
    (&mut *conn)
        .execute("SET LOCAL lc_monetary='C'")
        .await
        .map_err(|e| e.to_string())?;

    write(writer, "-- TupleDB PostgreSQL SQL export\n-- Portable schema/data export; source roles, grants and tablespaces are not copied.\n-- Restore into a selected database. Sequence state is not MVCC-transactional.\nSET client_encoding='UTF8';\nSET standard_conforming_strings=on;\nSET search_path=pg_catalog;\nSET check_function_bodies=false;\nSET DateStyle='ISO, YMD';\nSET IntervalStyle='postgres';\nSET TimeZone='UTC';\nSET extra_float_digits=3;\nSET bytea_output='hex';\n")?;
    if options.use_transactions {
        write(writer, "BEGIN;\n")?;
    }
    write(writer, "SET lc_monetary='C';\n")?;
    if structure {
        if options.drop_if_exists {
            for k in order.iter().rev() {
                let node = &plan.nodes[k];
                if !node.drop.is_empty() {
                    write(writer, &format!("{};\n", node.drop))?;
                }
            }
        }
        let mut schemas: BTreeSet<_> = order
            .iter()
            .map(|k| plan.nodes[k].schema.as_str())
            .collect();
        for seq in &plan.sequences {
            if seq.identity
                && seq
                    .owner
                    .as_ref()
                    .is_some_and(|(id, _)| selected.contains(id))
            {
                schemas.insert(seq.schema.as_str());
            }
        }
        for schema in schemas {
            write(
                writer,
                &format!("CREATE SCHEMA IF NOT EXISTS {};\n", ident(schema)?),
            )?;
            if let Some(Some(comment)) = plan.schema_comments.get(schema) {
                write(
                    writer,
                    &format!(
                        "COMMENT ON SCHEMA {} IS {};\n",
                        ident(schema)?,
                        literal(comment)
                    ),
                )?;
            }
        }
        for k in &order {
            let node = &plan.nodes[k];
            if !node.post {
                write(writer, &format!("{};\n", node.create.trim_end_matches(';')))?;
            }
        }
    }
    let mut total_rows = 0;
    if data {
        for (index, oid) in selected.iter().enumerate() {
            let relation = &plan.relations[oid];
            if relation.kind != "r" {
                continue;
            } // partitions exported ONLY once, views have no copied rows
            let qualified = sql::relation(&relation.reference)?;
            progress(index, selected.len(), format!("Exporting {qualified}…"));
            let columns = relation
                .columns
                .iter()
                .map(|c| ident(c))
                .collect::<Result<Vec<_>, _>>()?
                .join(", ");
            if columns.is_empty() {
                // Zero-column / generated-only tables still have meaningful row counts.
                let count: i64 =
                    sqlx::query_scalar(&format!("SELECT count(*) FROM ONLY {qualified}"))
                        .fetch_one(&mut *conn)
                        .await
                        .map_err(|e| e.to_string())?;
                for _ in 0..count {
                    write(
                        writer,
                        &format!("INSERT INTO {qualified} DEFAULT VALUES;\n"),
                    )?;
                    tokio::task::yield_now().await;
                }
                total_rows += count as usize;
                continue;
            }
            write(
                writer,
                &format!("COPY {qualified} ({columns}) FROM STDIN;\n"),
            )?;
            let copy_sql = format!(
                "COPY (SELECT {columns} FROM ONLY {qualified}) TO STDOUT WITH (FORMAT text)"
            );
            let mut stream = conn
                .copy_out_raw(&copy_sql)
                .await
                .map_err(|e| e.to_string())?;
            let mut rows = 0;
            let mut line_bytes = 0usize;
            while let Some(bytes) = stream.try_next().await.map_err(|e| e.to_string())? {
                // COPY text escapes embedded newlines, so physical lines count rows.
                let previous_rows = rows;
                for byte in &bytes {
                    line_bytes += 1;
                    if line_bytes > super::script::MAX_STATEMENT_BYTES {
                        return Err(format!("A COPY row in {qualified} exceeds the 16 MiB import limit; use pg_dump for this table"));
                    }
                    if *byte == b'\n' {
                        rows += 1;
                        line_bytes = 0;
                    }
                }
                writer.write_all(&bytes).map_err(|e| e.to_string())?;
                if rows / 5000 != previous_rows / 5000 {
                    progress(
                        index,
                        selected.len(),
                        format!("Exporting {qualified}: {rows} rows"),
                    );
                }
            }
            drop(stream);
            write(writer, "\\.\n")?;
            total_rows += rows;
        }
        for sequence in &plan.sequences {
            if sequence
                .owner
                .as_ref()
                .is_some_and(|(oid, _)| selected.contains(oid))
                || order.contains(&key("pg_class", sequence.oid))
            {
                let (last, called): (i64, bool) = sqlx::query_as(&format!(
                    "SELECT last_value, is_called FROM {}",
                    sequence.qualified
                ))
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| format!("Cannot read sequence {}: {e}", sequence.qualified))?;
                write(
                    writer,
                    &format!(
                        "SELECT pg_catalog.setval({}::regclass, {last}, {called});\n",
                        literal(&sequence.qualified)
                    ),
                )?;
            }
        }
    }
    if structure {
        for k in &order {
            let node = &plan.nodes[k];
            if node.post {
                write(writer, &format!("{};\n", node.create.trim_end_matches(';')))?;
            }
        }
        for sequence in &plan.sequences {
            if !sequence.identity && order.contains(&key("pg_class", sequence.oid)) {
                if let Some((oid, column)) = &sequence.owner {
                    if selected.contains(oid) {
                        write(
                            writer,
                            &format!(
                                "ALTER SEQUENCE {} OWNED BY {}.{};\n",
                                sequence.qualified,
                                sql::relation(&plan.relations[oid].reference)?,
                                ident(column)?
                            ),
                        )?;
                    }
                }
            }
        }
        for (k, comment) in &plan.comments {
            if order.contains(k) {
                write(writer, &format!("{comment};\n"))?;
            }
        }
    }
    if data {
        for k in &order {
            if k.0 == "pg_class" {
                if let Some(relation) = plan.relations.get(&k.1) {
                    if relation.kind == "m" && relation.populated {
                        write(
                            writer,
                            &format!(
                                "REFRESH MATERIALIZED VIEW {};\n",
                                sql::relation(&relation.reference)?
                            ),
                        )?;
                    }
                }
            }
        }
    }
    if options.use_transactions {
        write(writer, "COMMIT;\n")?;
    }
    (&mut *conn)
        .execute("ROLLBACK")
        .await
        .map_err(|e| e.to_string())?;
    Ok(total_rows)
}

impl Plan {
    fn resolve(&self, mut k: Key) -> Key {
        let mut seen = BTreeSet::new();
        while seen.insert(k.clone()) {
            match self.aliases.get(&k) {
                Some(next) => k = next.clone(),
                None => break,
            }
        }
        k
    }

    fn order(&self, selected: &BTreeSet<i64>, structure: bool) -> Result<Vec<Key>, String> {
        let mut needed: BTreeSet<Key> = selected.iter().map(|oid| key("pg_class", *oid)).collect();
        if structure && selected.len() == self.relations.len() {
            needed.extend(self.nodes.keys().cloned());
        }
        // Include post-data objects owned by selected relations and their functions/types.
        for (k, node) in &self.nodes {
            if node.post && node.owner.is_some_and(|oid| selected.contains(&oid)) {
                needed.insert(k.clone());
            }
        }
        for seq in &self.sequences {
            if !seq.identity
                && seq
                    .owner
                    .as_ref()
                    .is_some_and(|(id, _)| selected.contains(id))
            {
                needed.insert(key("pg_class", seq.oid));
            }
        }
        loop {
            let old = needed.len();
            for k in needed.clone() {
                let node = &self.nodes[&k];
                if node.create.len() > super::script::MAX_STATEMENT_BYTES {
                    return Err(format!(
                        "DDL for {} exceeds the 16 MiB import limit",
                        node.label
                    ));
                }
                if let Some(error) = &node.error {
                    return Err(format!(
                        "Cannot export {}: {error}. Use pg_dump for this object",
                        node.label
                    ));
                }
                if structure {
                    for dep in &node.deps {
                        if dep.0 == "pg_class"
                            && self.relations.contains_key(&dep.1)
                            && !selected.contains(&dep.1)
                        {
                            return Err(format!(
                                "{} depends on {}. Include that table/view in the export selection",
                                node.label, self.nodes[dep].label
                            ));
                        }
                        if self.nodes.contains_key(dep) {
                            needed.insert(dep.clone());
                        }
                    }
                }
            }
            if needed.len() == old {
                break;
            }
        }
        let mut done = BTreeSet::new();
        let mut order = Vec::new();
        while done.len() < needed.len() {
            let ready = needed
                .iter()
                .find(|k| {
                    !done.contains(*k)
                        && self.nodes[*k]
                            .deps
                            .iter()
                            .all(|d| !needed.contains(d) || done.contains(d))
                })
                .cloned();
            let Some(k) = ready else {
                return Err(
                    "Unsupported cyclic SQL object dependencies; use pg_dump for this schema"
                        .into(),
                );
            };
            done.insert(k.clone());
            order.push(k);
        }
        Ok(order)
    }

    async fn load(conn: &mut PgConnection, database: &str) -> Result<Self, String> {
        let mut plan = Self {
            nodes: BTreeMap::new(),
            aliases: BTreeMap::new(),
            relations: BTreeMap::new(),
            sequences: Vec::new(),
            schema_comments: BTreeMap::new(),
            comments: Vec::new(),
        };
        plan.load_sequences(conn).await?;
        plan.load_collations(conn).await?;
        plan.load_types(conn).await?;
        plan.load_functions(conn).await?;
        plan.load_relations(conn, database).await?;
        plan.load_post_data(conn).await?;
        plan.load_dependencies(conn).await?;
        Ok(plan)
    }

    async fn load_collations(&mut self, conn: &mut PgConnection) -> Result<(), String> {
        let rows = sqlx::query(
            "SELECT c.oid::bigint AS oid, n.nspname::text AS schema, c.collname::text AS name,
            c.collprovider::text AS provider, c.collisdeterministic, c.collcollate, c.collctype,
            COALESCE(to_jsonb(c)->>'colllocale',to_jsonb(c)->>'colliculocale') AS locale,
            to_jsonb(c)->>'collicurules' AS rules
            FROM pg_collation c JOIN pg_namespace n ON n.oid=c.collnamespace
            WHERE n.nspname !~ '^pg_' AND n.nspname<>'information_schema'",
        )
        .fetch_all(conn)
        .await
        .map_err(|e| e.to_string())?;
        for r in rows {
            let schema: String = r.get("schema");
            let qualified = name(&schema, r.get("name"))?;
            let provider: String = r.get("provider");
            let opts = match provider.as_str() {
                "c" => format!(
                    "PROVIDER = libc, LC_COLLATE = {}, LC_CTYPE = {}",
                    literal(r.get("collcollate")),
                    literal(r.get("collctype"))
                ),
                "i" | "b" => {
                    let mut opts = format!(
                        "PROVIDER = {}, LOCALE = {}",
                        if provider == "i" { "icu" } else { "builtin" },
                        literal(r.get("locale"))
                    );
                    if let Some(rules) = r.get::<Option<String>, _>("rules") {
                        opts += &format!(", RULES = {}", literal(&rules));
                    }
                    opts
                }
                "d" => String::new(),
                _ => return Err(format!("Unknown collation provider for {qualified}")),
            };
            let create = if provider == "d" {
                format!("CREATE COLLATION {qualified} FROM pg_catalog.\"default\"")
            } else {
                format!(
                    "CREATE COLLATION {qualified} ({opts}, DETERMINISTIC = {})",
                    r.get::<bool, _>("collisdeterministic")
                )
            };
            self.nodes.insert(
                key("pg_collation", r.get("oid")),
                Node::new(
                    &schema,
                    &qualified,
                    create,
                    format!("DROP COLLATION IF EXISTS {qualified}"),
                ),
            );
        }
        Ok(())
    }

    async fn load_sequences(&mut self, conn: &mut PgConnection) -> Result<(), String> {
        let rows = sqlx::query("SELECT c.oid::bigint AS oid, n.nspname::text AS schema, c.relname::text AS name, format_type(s.seqtypid,NULL) AS type,
            s.seqstart, s.seqincrement, s.seqmin, s.seqmax, s.seqcache, s.seqcycle,
            d.refobjid::bigint AS owner, a.attname::text AS column, COALESCE(d.deptype='i',false) AS identity
            FROM pg_sequence s JOIN pg_class c ON c.oid=s.seqrelid JOIN pg_namespace n ON n.oid=c.relnamespace
            LEFT JOIN pg_depend d ON d.classid='pg_class'::regclass AND d.objid=c.oid AND d.refclassid='pg_class'::regclass AND d.deptype IN ('a','i')
            LEFT JOIN pg_attribute a ON a.attrelid=d.refobjid AND a.attnum=d.refobjsubid
            WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'").fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
        for r in rows {
            let oid: i64 = r.get("oid");
            let schema: String = r.get("schema");
            let qualified = name(&schema, r.get("name"))?;
            let options = format!(
                "START WITH {} INCREMENT BY {} MINVALUE {} MAXVALUE {} CACHE {} {}CYCLE",
                r.get::<i64, _>("seqstart"),
                r.get::<i64, _>("seqincrement"),
                r.get::<i64, _>("seqmin"),
                r.get::<i64, _>("seqmax"),
                r.get::<i64, _>("seqcache"),
                if r.get::<bool, _>("seqcycle") {
                    ""
                } else {
                    "NO "
                }
            );
            let owner = r
                .get::<Option<i64>, _>("owner")
                .zip(r.get::<Option<String>, _>("column"));
            let identity: bool = r.get("identity");
            if identity {
                if let Some((owner, _)) = &owner {
                    self.aliases
                        .insert(key("pg_class", oid), key("pg_class", *owner));
                }
            } else {
                self.nodes.insert(
                    key("pg_class", oid),
                    Node::new(
                        &schema,
                        &qualified,
                        format!(
                            "CREATE SEQUENCE {qualified} AS {} {options}",
                            r.get::<String, _>("type")
                        ),
                        format!("DROP SEQUENCE IF EXISTS {qualified}"),
                    ),
                );
            }
            self.sequences.push(Sequence {
                schema,
                oid,
                qualified,
                owner,
                identity,
                options,
            });
        }
        Ok(())
    }

    async fn load_types(&mut self, conn: &mut PgConnection) -> Result<(), String> {
        let rows = sqlx::query("SELECT t.oid::bigint AS oid, n.nspname::text AS schema, t.typname::text AS name, t.typtype::text AS kind,
            t.typrelid::bigint AS relation, COALESCE(c.relkind::text,'') AS relkind, t.typelem::bigint AS element, t.typarray::bigint AS array,
            format_type(t.typbasetype,t.typtypmod) AS base, t.typnotnull, pg_get_expr(t.typdefaultbin,0) AS typdefault,
            CASE WHEN t.typcollation<>0 THEN t.typcollation::regcollation::text END AS collation
            FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace LEFT JOIN pg_class c ON c.oid=t.typrelid
            WHERE n.nspname !~ '^pg_' AND n.nspname <> 'information_schema'").fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
        for r in rows {
            let oid: i64 = r.get("oid");
            let schema: String = r.get("schema");
            let kind: String = r.get("kind");
            let qualified = name(&schema, r.get("name"))?;
            let array: i64 = r.get("array");
            if array != 0 {
                self.aliases
                    .insert(key("pg_type", array), key("pg_type", oid));
            }
            if r.get::<i64, _>("element") != 0 {
                continue;
            }
            if kind == "c" && r.get::<String, _>("relkind") != "c" {
                self.aliases
                    .insert(key("pg_type", oid), key("pg_class", r.get("relation")));
                continue;
            }
            if kind == "m" {
                continue;
            } // mapped to its range below
            let mut node = Node::new(
                &schema,
                &qualified,
                String::new(),
                format!(
                    "DROP {} IF EXISTS {qualified}",
                    if kind == "d" { "DOMAIN" } else { "TYPE" }
                ),
            );
            node.create = match kind.as_str() {
                "e" => {
                    let values: Vec<String> = sqlx::query_scalar("SELECT enumlabel::text FROM pg_enum WHERE enumtypid=$1::oid ORDER BY enumsortorder").bind(oid).fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
                    format!(
                        "CREATE TYPE {qualified} AS ENUM ({})",
                        values
                            .iter()
                            .map(|s| literal(s))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
                "d" => {
                    let mut ddl = format!(
                        "CREATE DOMAIN {qualified} AS {}",
                        r.get::<String, _>("base")
                    );
                    if let Some(collation) = r.get::<Option<String>, _>("collation") {
                        ddl += &format!(" COLLATE {collation}");
                    }
                    if let Some(default) = r.get::<Option<String>, _>("typdefault") {
                        // Deparse the expression tree under pg_catalog instead
                        // of using the stale, search_path-dependent typdefault.
                        ddl += &format!(" DEFAULT {default}");
                    }
                    if r.get::<bool, _>("typnotnull") {
                        ddl += " NOT NULL";
                    }
                    let constraints = sqlx::query("SELECT conname::text AS name, pg_get_constraintdef(oid,true) AS def FROM pg_constraint WHERE contypid=$1::oid ORDER BY conname").bind(oid).fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
                    for c in constraints {
                        ddl += &format!(
                            " CONSTRAINT {} {}",
                            ident(c.get("name"))?,
                            c.get::<String, _>("def")
                        );
                    }
                    ddl
                }
                "c" => {
                    let rel: i64 = r.get("relation");
                    self.aliases
                        .insert(key("pg_class", rel), key("pg_type", oid));
                    let cols = sqlx::query("SELECT attname::text AS name, format_type(atttypid,atttypmod) AS type, CASE WHEN attcollation<>0 THEN attcollation::regcollation::text END AS collation FROM pg_attribute WHERE attrelid=$1::oid AND attnum>0 AND NOT attisdropped ORDER BY attnum").bind(rel).fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
                    let defs = cols
                        .iter()
                        .map(|c| {
                            Ok(format!(
                                "{} {}{}",
                                ident(c.get("name"))?,
                                c.get::<String, _>("type"),
                                c.get::<Option<String>, _>("collation")
                                    .map(|v| format!(" COLLATE {v}"))
                                    .unwrap_or_default()
                            ))
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    format!("CREATE TYPE {qualified} AS ({})", defs.join(", "))
                }
                "r" => {
                    let range = sqlx::query("SELECT format_type(rngsubtype,NULL) AS subtype, rngmultitypid::bigint AS multi, rngmultitypid::regtype::text AS multiname, rngcanonical::oid::bigint AS canonical, rngsubdiff::oid::bigint AS diff, rngsubdiff::regproc::text AS diffname, CASE WHEN rngcollation<>0 THEN rngcollation::regcollation::text END AS collation, o.opcname::text AS opclass, n.nspname::text AS opschema FROM pg_range JOIN pg_opclass o ON o.oid=rngsubopc JOIN pg_namespace n ON n.oid=o.opcnamespace WHERE rngtypid=$1::oid").bind(oid).fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
                    self.aliases
                        .insert(key("pg_type", range.get("multi")), key("pg_type", oid));
                    if range.get::<i64, _>("canonical") != 0 {
                        node.error = Some("range canonical functions require shell types".into());
                    }
                    let mut opts = vec![
                        format!("SUBTYPE = {}", range.get::<String, _>("subtype")),
                        format!(
                            "SUBTYPE_OPCLASS = {}",
                            name(range.get("opschema"), range.get("opclass"))?
                        ),
                        format!(
                            "MULTIRANGE_TYPE_NAME = {}",
                            range.get::<String, _>("multiname")
                        ),
                    ];
                    if range.get::<i64, _>("diff") != 0 {
                        opts.push(format!(
                            "SUBTYPE_DIFF = {}",
                            range.get::<String, _>("diffname")
                        ));
                    }
                    if let Some(collation) = range.get::<Option<String>, _>("collation") {
                        opts.push(format!("COLLATION = {collation}"));
                    }
                    format!("CREATE TYPE {qualified} AS RANGE ({})", opts.join(", "))
                }
                _ => {
                    node.error =
                        Some("custom base types are not supported by native SQL export".into());
                    String::new()
                }
            };
            self.nodes.insert(key("pg_type", oid), node);
        }
        Ok(())
    }

    async fn load_functions(&mut self, conn: &mut PgConnection) -> Result<(), String> {
        let rows = sqlx::query("SELECT p.oid::bigint AS oid, n.nspname::text AS schema, p.proname::text AS name, p.prokind::text AS kind,
            pg_get_function_identity_arguments(p.oid) AS args,
            CASE WHEN p.prokind <> 'a' THEN pg_get_functiondef(p.oid) ELSE '' END AS def,
            p.prosqlbody IS NOT NULL AS atomic
            FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace WHERE n.nspname !~ '^pg_' AND n.nspname<>'information_schema'").fetch_all(conn).await.map_err(|e| e.to_string())?;
        for r in rows {
            let schema: String = r.get("schema");
            let qualified = name(&schema, r.get("name"))?;
            let kind = if r.get::<String, _>("kind") == "p" {
                "PROCEDURE"
            } else {
                "FUNCTION"
            };
            let mut node = Node::new(
                &schema,
                &qualified,
                r.get("def"),
                format!(
                    "DROP {kind} IF EXISTS {qualified}({})",
                    r.get::<String, _>("args")
                ),
            );
            if r.get::<String, _>("kind") == "a" || r.get::<bool, _>("atomic") {
                node.error = Some(
                    "aggregates and SQL-standard function bodies are not supported yet".into(),
                );
            }
            self.nodes.insert(key("pg_proc", r.get("oid")), node);
        }
        Ok(())
    }

    async fn load_relations(
        &mut self,
        conn: &mut PgConnection,
        database: &str,
    ) -> Result<(), String> {
        let rows = sqlx::query("SELECT c.oid::bigint AS oid, n.nspname::text AS schema, c.relname::text AS name, c.relkind::text AS kind,
            c.relpersistence::text AS persistence, c.reloptions, c.relrowsecurity OR c.relforcerowsecurity AS rls,
            c.reloftype::bigint AS oftype, c.relreplident::text AS replica, c.relispopulated,
            c.relispartition, pg_get_partkeydef(c.oid) AS partkey, pg_get_expr(c.relpartbound,c.oid) AS bound,
            (SELECT inhparent::bigint FROM pg_inherits WHERE inhrelid=c.oid LIMIT 1) AS parent,
            CASE WHEN c.relkind IN ('v','m') THEN pg_get_viewdef(c.oid,false) END AS viewdef,
            obj_description(n.oid,'pg_namespace') AS schema_comment
            FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
            WHERE c.relkind IN ('r','p','v','m','f') AND n.nspname !~ '^pg_' AND n.nspname<>'information_schema'").fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
        for r in rows {
            let oid: i64 = r.get("oid");
            let schema: String = r.get("schema");
            let object: String = r.get("name");
            let kind: String = r.get("kind");
            let qualified = name(&schema, &object)?;
            self.schema_comments
                .insert(schema.clone(), r.get("schema_comment"));
            let object_kind = match kind.as_str() {
                "v" => "VIEW",
                "m" => "MATERIALIZED VIEW",
                _ => "TABLE",
            };
            let mut node = Node::new(
                &schema,
                &qualified,
                String::new(),
                format!("DROP {object_kind} IF EXISTS {qualified}"),
            );
            if kind == "f"
                || r.get::<bool, _>("rls")
                || r.get::<i64, _>("oftype") != 0
                || (kind == "r" || kind == "p") && r.get::<String, _>("replica") != "d"
            {
                node.error = Some("foreign/typed tables, row security and custom replica identity are not supported yet".into());
            }
            let relopts = r
                .get::<Option<Vec<String>>, _>("reloptions")
                .unwrap_or_default();
            let storage = if relopts.is_empty() {
                String::new()
            } else {
                format!(
                    " WITH ({})",
                    relopts
                        .iter()
                        .map(|s| {
                            let (key, value) = s.split_once('=').ok_or("Invalid storage option")?;
                            // Names are server-generated, but still quote each namespace component.
                            Ok(format!(
                                "{} = {}",
                                key.split('.')
                                    .map(ident)
                                    .collect::<Result<Vec<_>, _>>()?
                                    .join("."),
                                literal(value)
                            ))
                        })
                        .collect::<Result<Vec<_>, String>>()?
                        .join(", ")
                )
            };
            let cols = sqlx::query("SELECT a.attname::text AS name, format_type(a.atttypid,a.atttypmod) AS type,
                a.attnotnull, a.attidentity::text AS identity, a.attgenerated::text AS generated,
                d.oid::bigint AS default_oid, pg_get_expr(d.adbin,d.adrelid) AS def, CASE WHEN a.attcollation<>0 THEN a.attcollation::regcollation::text END AS collation,
                col_description(a.attrelid,a.attnum) AS comment
                FROM pg_attribute a LEFT JOIN pg_attrdef d ON d.adrelid=a.attrelid AND d.adnum=a.attnum
                WHERE a.attrelid=$1::oid AND a.attnum>0 AND NOT a.attisdropped ORDER BY a.attnum").bind(oid).fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
            let mut definitions = Vec::new();
            let mut copy_columns = Vec::new();
            for c in cols {
                let column: String = c.get("name");
                let generated: String = c.get("generated");
                let identity: String = c.get("identity");
                if kind == "v" {
                    if let Some(default) = c.get::<Option<String>, _>("def") {
                        let column = ident(&column)?;
                        // CREATE VIEW only preserves its SELECT, not column
                        // defaults. Keep defaults separate so a default function
                        // can itself depend on the view's row type without a cycle.
                        let mut default_node = Node::new(
                            &schema,
                            &format!("{qualified}.{column} default"),
                            format!("ALTER VIEW {qualified} ALTER COLUMN {column} SET DEFAULT {default}"),
                            format!("ALTER VIEW IF EXISTS {qualified} ALTER COLUMN {column} DROP DEFAULT"),
                        );
                        default_node.post = true;
                        default_node.owner = Some(oid);
                        default_node.deps.insert(key("pg_class", oid));
                        self.nodes
                            .insert(key("pg_attrdef", c.get("default_oid")), default_node);
                    }
                }
                if generated.is_empty() {
                    copy_columns.push(column.clone());
                }
                let mut ddl = format!("{} {}", ident(&column)?, c.get::<String, _>("type"));
                if let Some(collation) = c.get::<Option<String>, _>("collation") {
                    ddl += &format!(" COLLATE {collation}");
                }
                // Identity on a partition is inherited from its root, including
                // the root's sequence. PARTITION OF recreates that relationship.
                if !identity.is_empty() && !r.get::<bool, _>("relispartition") {
                    let seq = self
                        .sequences
                        .iter()
                        .find(|s| s.identity && s.owner.as_ref() == Some(&(oid, column.clone())))
                        .ok_or_else(|| {
                            format!("Missing identity sequence for {qualified}.{column}")
                        })?;
                    ddl += &format!(
                        " GENERATED {} AS IDENTITY (SEQUENCE NAME {} {})",
                        if identity == "a" {
                            "ALWAYS"
                        } else {
                            "BY DEFAULT"
                        },
                        seq.qualified,
                        seq.options
                    );
                } else if let Some(default) = c.get::<Option<String>, _>("def") {
                    ddl += &if generated.is_empty() {
                        format!(" DEFAULT {default}")
                    } else {
                        format!(
                            " GENERATED ALWAYS AS ({default}) {}",
                            if generated == "s" {
                                "STORED"
                            } else {
                                "VIRTUAL"
                            }
                        )
                    };
                }
                if c.get::<bool, _>("attnotnull") {
                    ddl += " NOT NULL";
                }
                definitions.push(ddl);
                if let Some(comment) = c.get::<Option<String>, _>("comment") {
                    self.comments.push((
                        key("pg_class", oid),
                        format!(
                            "COMMENT ON COLUMN {qualified}.{} IS {}",
                            ident(&column)?,
                            literal(&comment)
                        ),
                    ));
                }
            }
            node.create = if kind == "v" || kind == "m" {
                format!(
                    "CREATE {object_kind} {qualified}{storage} AS {}{}",
                    r.get::<String, _>("viewdef").trim_end_matches(';'),
                    if kind == "m" { " WITH NO DATA" } else { "" }
                )
            } else if r.get::<bool, _>("relispartition") {
                let parent: i64 = r.get("parent");
                let overrides: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_attribute a JOIN pg_attribute p ON p.attrelid=$2::oid AND p.attname=a.attname LEFT JOIN pg_attrdef ad ON ad.adrelid=a.attrelid AND ad.adnum=a.attnum LEFT JOIN pg_attrdef pd ON pd.adrelid=p.attrelid AND pd.adnum=p.attnum WHERE a.attrelid=$1::oid AND a.attnum>0 AND NOT a.attisdropped AND (a.attnotnull<>p.attnotnull OR a.attgenerated<>p.attgenerated OR pg_get_expr(ad.adbin,ad.adrelid) IS DISTINCT FROM pg_get_expr(pd.adbin,pd.adrelid)))").bind(oid).bind(parent).fetch_one(&mut *conn).await.map_err(|e| e.to_string())?;
                if overrides {
                    node.error =
                        Some("partition-specific column overrides are not supported yet".into());
                }
                node.deps.insert(key("pg_class", parent));
                let parent_name: String = sqlx::query_scalar(
                    "SELECT oid::regclass::text FROM pg_class WHERE oid=$1::oid",
                )
                .bind(parent)
                .fetch_one(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
                // Non-default partition column overrides need explicit support.
                format!(
                    "CREATE TABLE {qualified} PARTITION OF {parent_name} {}{}{storage}",
                    r.get::<String, _>("bound"),
                    r.get::<Option<String>, _>("partkey")
                        .map(|s| format!(" PARTITION BY {s}"))
                        .unwrap_or_default()
                )
            } else {
                if r.get::<Option<i64>, _>("parent").is_some() {
                    node.error = Some("legacy table inheritance is not supported yet".into());
                }
                format!(
                    "CREATE {}TABLE {qualified} (\n  {}\n){}{storage}",
                    if r.get::<String, _>("persistence") == "u" {
                        "UNLOGGED "
                    } else {
                        ""
                    },
                    definitions.join(",\n  "),
                    r.get::<Option<String>, _>("partkey")
                        .map(|s| format!(" PARTITION BY {s}"))
                        .unwrap_or_default()
                )
            };
            self.nodes.insert(key("pg_class", oid), node);
            self.relations.insert(
                oid,
                Relation {
                    reference: TableRef {
                        catalog: database.into(),
                        schema: Some(schema),
                        name: object,
                    },
                    kind,
                    columns: copy_columns,
                    populated: r.get("relispopulated"),
                    parent: if r.get::<bool, _>("relispartition") {
                        r.get("parent")
                    } else {
                        None
                    },
                },
            );
        }
        Ok(())
    }

    async fn load_post_data(&mut self, conn: &mut PgConnection) -> Result<(), String> {
        let constraints = sqlx::query("SELECT oid::bigint AS oid, conrelid::bigint AS relation, conname::text AS name, pg_get_constraintdef(oid,false) AS def FROM pg_constraint WHERE conrelid<>0 AND conparentid=0 AND conislocal AND contype IN ('c','p','u','f','x')").fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
        for r in constraints {
            let owner: i64 = r.get("relation");
            let Some(relation) = self.relations.get(&owner) else {
                continue;
            };
            let qualified = sql::relation(&relation.reference)?;
            let constraint = ident(r.get("name"))?;
            let mut node = Node::new(
                relation.reference.schema.as_deref().unwrap(),
                &format!("{qualified}.{constraint}"),
                format!(
                    "ALTER TABLE {qualified} ADD CONSTRAINT {constraint} {}",
                    r.get::<String, _>("def")
                ),
                format!("ALTER TABLE IF EXISTS {qualified} DROP CONSTRAINT IF EXISTS {constraint}"),
            );
            node.post = true;
            node.owner = Some(owner);
            node.deps.insert(key("pg_class", owner));
            self.nodes.insert(key("pg_constraint", r.get("oid")), node);
        }
        let indexes=sqlx::query("SELECT i.indexrelid::bigint AS oid, i.indrelid::bigint AS relation, pg_get_indexdef(i.indexrelid) AS def, i.indexrelid::regclass::text AS name, i.indisvalid AND i.indisready AS valid, c.relkind='I' AS partitioned, (SELECT inhparent::bigint FROM pg_inherits WHERE inhrelid=i.indexrelid LIMIT 1) AS parent FROM pg_index i JOIN pg_class c ON c.oid=i.indexrelid WHERE NOT EXISTS(SELECT 1 FROM pg_constraint WHERE conindid=i.indexrelid AND contype IN ('p','u','x'))").fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
        let mut attachments = Vec::new();
        for r in indexes {
            let owner: i64 = r.get("relation");
            let Some(relation) = self.relations.get(&owner) else {
                continue;
            };
            let qualified: String = r.get("name");
            let mut node = Node::new(
                relation.reference.schema.as_deref().unwrap(),
                &qualified,
                r.get("def"),
                format!("DROP INDEX IF EXISTS {qualified}"),
            );
            if !r.get::<bool, _>("valid") && !r.get::<bool, _>("partitioned") {
                node.error = Some("invalid physical indexes cannot be restored faithfully".into());
            }
            if let Some(parent) = r.get::<Option<i64>, _>("parent") {
                // Dropping the root removes attached indexes; PostgreSQL does
                // not allow dropping an attached child independently.
                node.drop.clear();
                node.deps.insert(key("pg_class", parent));
                attachments.push((r.get::<i64, _>("oid"), parent, owner, qualified.clone()));
            }
            node.post = true;
            node.owner = Some(owner);
            node.deps.insert(key("pg_class", owner));
            self.nodes.insert(key("pg_class", r.get("oid")), node);
        }
        for (oid, parent, owner, qualified) in &attachments {
            let parent_key = key("pg_class", *parent);
            let parent_node = self
                .nodes
                .get(&parent_key)
                .ok_or("Missing parent index definition")?;
            let mut node = Node::new(
                &parent_node.schema,
                &format!("attachment of {qualified}"),
                format!(
                    "ALTER INDEX {} ATTACH PARTITION {qualified}",
                    parent_node.label
                ),
                String::new(),
            );
            node.post = true;
            node.owner = Some(*owner);
            node.deps.extend([parent_key, key("pg_class", *oid)]);
            // Nested partitioned indexes must be complete before attachment.
            node.deps.extend(
                attachments
                    .iter()
                    .filter(|(_, p, _, _)| p == oid)
                    .map(|(child, _, _, _)| key("index_attachment", *child)),
            );
            self.nodes.insert(key("index_attachment", *oid), node);
        }
        let triggers=sqlx::query("SELECT oid::bigint AS oid, tgrelid::bigint AS relation, tgfoid::bigint AS function, tgname::text AS name, pg_get_triggerdef(oid,false) AS def, tgenabled::text AS enabled FROM pg_trigger WHERE NOT tgisinternal AND tgparentid=0").fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
        for r in triggers {
            let owner: i64 = r.get("relation");
            let Some(relation) = self.relations.get(&owner) else {
                continue;
            };
            let qualified = sql::relation(&relation.reference)?;
            let trigger = ident(r.get("name"))?;
            let mut ddl: String = r.get("def");
            let enabled: String = r.get("enabled");
            if enabled != "O" {
                ddl += &format!(
                    ";\nALTER TABLE {qualified} {} TRIGGER {trigger}",
                    match enabled.as_str() {
                        "D" => "DISABLE",
                        "R" => "ENABLE REPLICA",
                        _ => "ENABLE ALWAYS",
                    }
                );
            }
            let mut node = Node::new(
                relation.reference.schema.as_deref().unwrap(),
                &format!("{qualified}.{trigger}"),
                ddl,
                format!("DROP TRIGGER IF EXISTS {trigger} ON {qualified}"),
            );
            // DROP TRIGGER lacks an IF EXISTS guard for a missing owning table;
            // the table drop itself removes it, so no separate drop is necessary.
            node.drop.clear();
            node.post = true;
            node.owner = Some(owner);
            node.deps.insert(key("pg_class", owner));
            node.deps.insert(key("pg_proc", r.get("function")));
            self.nodes.insert(key("pg_trigger", r.get("oid")), node);
        }
        Ok(())
    }

    async fn load_dependencies(&mut self, conn: &mut PgConnection) -> Result<(), String> {
        let aliases=sqlx::query("SELECT 'pg_rewrite' AS class, oid::bigint AS oid, 'pg_class' AS refclass, ev_class::bigint AS refid FROM pg_rewrite WHERE rulename='_RETURN'
            UNION ALL SELECT 'pg_attrdef',oid::bigint,'pg_class',adrelid::bigint FROM pg_attrdef
            UNION ALL SELECT 'pg_constraint',oid::bigint,'pg_type',contypid::bigint FROM pg_constraint WHERE contypid<>0
            UNION ALL SELECT 'pg_class',conindid::bigint,'pg_class',conrelid::bigint FROM pg_constraint WHERE conindid<>0 AND contype IN ('p','u','x')").fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
        for r in aliases {
            let k = key(r.get("class"), r.get("oid"));
            // View defaults have their own ALTER VIEW node and dependencies.
            // Table defaults remain inline in CREATE TABLE.
            if k.0 == "pg_attrdef" && self.nodes.contains_key(&k) {
                continue;
            }
            self.aliases
                .insert(k, key(r.get("refclass"), r.get("refid")));
        }
        let deps=sqlx::query("SELECT classid::regclass::text AS class, objid::bigint AS oid, refclassid::regclass::text AS refclass, refobjid::bigint AS refid, deptype::text AS kind FROM pg_depend").fetch_all(&mut *conn).await.map_err(|e| e.to_string())?;
        // Range/multirange constructors belong internally to their type. CREATE
        // TYPE recreates them; exporting/dropping them separately is invalid.
        for r in &deps {
            if r.get::<&str, _>("class") == "pg_proc"
                && r.get::<&str, _>("refclass") == "pg_type"
                && r.get::<&str, _>("kind") == "i"
            {
                let function = key("pg_proc", r.get("oid"));
                self.nodes.remove(&function);
                self.aliases
                    .insert(function, key("pg_type", r.get("refid")));
            }
        }
        for r in deps {
            let raw = key(r.get("class"), r.get("oid"));
            let k = self.resolve(raw.clone());
            let dep = self.resolve(key(r.get("refclass"), r.get("refid")));
            // A constraint-owned index is created by ADD CONSTRAINT, not by
            // CREATE TABLE. Its internal edge must not make the table depend
            // on its own future primary/unique constraint.
            if raw.0 == "pg_class" && raw != k && dep.0 == "pg_constraint" {
                continue;
            }
            if r.get::<String, _>("kind") == "e" {
                if let Some(node) = self.nodes.get_mut(&k) {
                    node.error =
                        Some("extension-owned objects require an extension-aware dump".into());
                }
            }
            // Sequence ownership is applied after tables/defaults; it is not a
            // creation prerequisite and would introduce an artificial cycle.
            if self
                .sequences
                .iter()
                .any(|s| !s.identity && raw == key("pg_class", s.oid))
                && r.get::<String, _>("kind") == "a"
            {
                continue;
            }
            if k != dep && self.nodes.contains_key(&dep) {
                if let Some(node) = self.nodes.get_mut(&k) {
                    node.deps.insert(dep);
                }
            }
            if let Some(node) = self.nodes.get_mut(&k) {
                if [
                    "pg_operator",
                    "pg_opclass",
                    "pg_conversion",
                    "pg_ts_config",
                    "pg_ts_dict",
                ]
                .contains(&r.get::<&str, _>("refclass"))
                    && r.get::<i64, _>("refid") >= 16384
                {
                    node.error = Some(
                        "a custom operator/text-search dependency is not supported yet".into(),
                    );
                }
            }
        }
        // User rules change writes and cannot be silently omitted.
        let rules: Vec<i64> =
            sqlx::query_scalar("SELECT ev_class::bigint FROM pg_rewrite WHERE rulename<>'_RETURN'")
                .fetch_all(&mut *conn)
                .await
                .map_err(|e| e.to_string())?;
        for oid in rules {
            if let Some(node) = self.nodes.get_mut(&key("pg_class", oid)) {
                node.error = Some("user-defined rewrite rules are not supported yet".into());
            }
        }
        let comments=sqlx::query("SELECT classoid::regclass::text AS class, objoid::bigint AS oid, description FROM pg_description WHERE objsubid=0").fetch_all(conn).await.map_err(|e|e.to_string())?;
        for r in comments {
            let k = key(r.get("class"), r.get("oid"));
            if let Some(node) = self.nodes.get(&k) {
                let object = match k.0.as_str() {
                    "pg_collation" => "COLLATION",
                    "pg_class" => self
                        .relations
                        .get(&k.1)
                        .map(|r| match r.kind.as_str() {
                            "v" => "VIEW",
                            "m" => "MATERIALIZED VIEW",
                            _ => "TABLE",
                        })
                        .unwrap_or(if self.sequences.iter().any(|s| s.oid == k.1) {
                            "SEQUENCE"
                        } else {
                            "INDEX"
                        }),
                    "pg_type" => {
                        if node.create.starts_with("CREATE DOMAIN") {
                            "DOMAIN"
                        } else {
                            "TYPE"
                        }
                    }
                    _ => continue,
                };
                self.comments.push((
                    k,
                    format!(
                        "COMMENT ON {object} {} IS {}",
                        node.label,
                        literal(r.get("description"))
                    ),
                ));
            }
        }
        Ok(())
    }
}
