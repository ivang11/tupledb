use super::*;
use sqlx::Row;

#[async_trait]
impl CatalogDriver for PostgreSqlDriver {
    async fn get_databases(&self) -> Result<Vec<String>, String> {
        let pool = self.pool_for(&self.catalog).await?;
        sqlx::query_scalar(
            "SELECT datname::text FROM pg_catalog.pg_database
            WHERE datallowconn AND NOT datistemplate AND has_database_privilege(oid, 'CONNECT')
            ORDER BY datname",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())
    }
    async fn get_database_creation_options(&self) -> Result<DatabaseCreationOptions, String> {
        let pool = self.pool_for(&self.catalog).await?;
        Ok(database_options::Options::load(&pool).await?.public())
    }
    async fn create_database(
        &self,
        name: &str,
        character_set: Option<&str>,
        collation: Option<&str>,
    ) -> Result<(), String> {
        self.check_write(false)?;
        sql::quote_identifier(name)?;
        let pool = self.pool_for(&self.catalog).await?;
        let query = if character_set.is_none() && collation.is_none() {
            format!("CREATE DATABASE {}", sql::quote_identifier(name)?)
        } else {
            database_options::Options::load(&pool).await?.create_sql(
                name,
                character_set,
                collation,
            )?
        };
        // CREATE DATABASE must run outside an explicit transaction.
        sqlx::query(&query)
            .execute(&pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    async fn drop_database(&self, name: &str) -> Result<(), String> {
        self.check_write(false)?;
        let quoted = sql::quote_identifier(name)?;
        if name == self.catalog || name == "template0" || name == "template1" {
            return Err("Cannot delete this connection's initial database or a template database; reconnect using another initial database".into());
        }
        let admin = self.pool_for(&self.catalog).await?;
        // Keep new sessions from racing deletion. Close only our target pool;
        // other clients remain protected by PostgreSQL's normal DROP checks.
        // Remove it from the map before closing so unrelated catalogs' lookups
        // aren't blocked on this pool's drain.
        let removed = {
            let mut state = self.pools.lock().await;
            let pools = state.as_mut().ok_or("PostgreSQL connection is closed")?;
            pools.remove(name)
        };
        if let Some(pool) = removed {
            pool.close().await;
        }
        sqlx::query(&format!("DROP DATABASE {quoted}"))
            .execute(&admin)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn get_tables(&self, database: &str) -> Result<Vec<Table>, String> {
        let pool = self.pool_for(database).await?;
        let rows = sqlx::query(
            "SELECT n.nspname::text AS schema, c.relname::text AS name, c.relkind::text AS kind
            FROM pg_catalog.pg_class c JOIN pg_catalog.pg_namespace n ON n.oid=c.relnamespace
            WHERE c.relkind IN ('r','p','v','m','f') AND n.nspname <> 'information_schema'
            AND n.nspname !~ '^pg_' AND has_schema_privilege(n.oid,'USAGE')
            AND has_table_privilege(c.oid,'SELECT') ORDER BY n.nspname,c.relname",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| e.to_string())?;
        Ok(rows
            .iter()
            .map(|r| {
                let name: String = r.get("name");
                let kind: &str = r.get("kind");
                Table {
                    reference: TableRef {
                        catalog: database.into(),
                        schema: Some(r.get("schema")),
                        name: name.clone(),
                    },
                    name,
                    table_type: match kind {
                        "v" => "VIEW",
                        "m" => "MATERIALIZED VIEW",
                        "f" => "FOREIGN TABLE",
                        _ => "BASE TABLE",
                    }
                    .into(),
                }
            })
            .collect())
    }

    async fn get_table_structure(&self, table: &TableRef) -> Result<Vec<ColumnStructure>, String> {
        Ok(self
            .column_metadata(table)
            .await?
            .into_iter()
            .map(|c| c.structure)
            .collect())
    }

    async fn get_table_ddl(&self, table: &TableRef) -> Result<String, String> {
        self.inspect_table_ddl(table).await
    }

    async fn get_foreign_keys(&self, table: &TableRef) -> Result<Vec<ForeignKey>, String> {
        let relation = self.relation(table)?;
        let pool = self.pool_for(&table.catalog).await?;
        let rows = sqlx::query("SELECT c.conname::text AS constraint_name, k.position::int AS position,
                a.attname::text AS column, n.nspname::text AS schema, r.relname::text AS table,
                b.attname::text AS referenced_column
            FROM pg_constraint c
            CROSS JOIN LATERAL unnest(c.conkey,c.confkey) WITH ORDINALITY k(local_num,foreign_num,position)
            JOIN pg_attribute a ON a.attrelid=c.conrelid AND a.attnum=k.local_num
            JOIN pg_attribute b ON b.attrelid=c.confrelid AND b.attnum=k.foreign_num
            JOIN pg_class r ON r.oid=c.confrelid JOIN pg_namespace n ON n.oid=r.relnamespace
            WHERE c.conrelid=$1::regclass AND c.contype='f' ORDER BY c.conname,k.position")
            .bind(relation).fetch_all(&pool).await.map_err(|e| e.to_string())?;
        Ok(rows
            .iter()
            .map(|r| ForeignKey {
                constraint_name: r.get("constraint_name"),
                position: r.get::<i32, _>("position") as usize,
                column: r.get("column"),
                referenced_table: r.get("table"),
                referenced_column: r.get("referenced_column"),
                referenced: TableRef {
                    catalog: table.catalog.clone(),
                    schema: Some(r.get("schema")),
                    name: r.get("table"),
                },
            })
            .collect())
    }

    async fn get_table_indexes(&self, table: &TableRef) -> Result<Vec<TableIndex>, String> {
        let relation = self.relation(table)?;
        let pool = self.pool_for(&table.catalog).await?;
        let rows = sqlx::query("SELECT r.relname::text AS name, NOT i.indisunique AS non_unique,
                pg_get_indexdef(i.indexrelid,k.position::int,true) AS column, k.position::bigint AS position,
                am.amname::text AS method, COALESCE(NOT a.attnotnull,true) AS nullable,
                COALESCE(pg_get_expr(i.indpred,i.indrelid),'') AS predicate
            FROM pg_index i JOIN pg_class r ON r.oid=i.indexrelid JOIN pg_am am ON am.oid=r.relam
            CROSS JOIN LATERAL unnest(i.indkey) WITH ORDINALITY k(attnum,position)
            LEFT JOIN pg_attribute a ON a.attrelid=i.indrelid AND a.attnum=k.attnum
            WHERE i.indrelid=$1::regclass ORDER BY r.relname,k.position")
            .bind(relation).fetch_all(&pool).await.map_err(|e| e.to_string())?;
        Ok(rows
            .iter()
            .map(|r| TableIndex {
                key_name: r.get("name"),
                non_unique: r.get("non_unique"),
                column_name: r.get("column"),
                seq_in_index: r.get::<i64, _>("position") as u64,
                index_type: r.get("method"),
                nullable: r.get("nullable"),
                comment: r.get("predicate"),
            })
            .collect())
    }

    async fn get_primary_key_columns(&self, table: &TableRef) -> Result<Vec<String>, String> {
        let mut columns = self.get_table_structure(table).await?;
        columns.retain(|c| c.primary_key_position.is_some());
        columns.sort_by_key(|c| c.primary_key_position);
        Ok(columns.into_iter().map(|c| c.field).collect())
    }

    async fn get_estimated_row_count(&self, table: &TableRef) -> Result<i64, String> {
        let pool = self.pool_for(&table.catalog).await?;
        sqlx::query_scalar(
            "SELECT GREATEST(reltuples,0)::bigint FROM pg_class WHERE oid=$1::regclass",
        )
        .bind(self.relation(table)?)
        .fetch_one(&pool)
        .await
        .map_err(|e| e.to_string())
    }
}

impl PostgreSqlDriver {
    // Peel domain/array layers to a fully qualified input type without typmods.
    // The destination applies length, precision and domain checks implicitly.
    pub(super) async fn column_metadata(&self, table: &TableRef) -> Result<Vec<PgColumn>, String> {
        let relation = self.relation(table)?;
        let pool = self.pool_for(&table.catalog).await?;
        // Only GENERATED ALWAYS AS IDENTITY ('a') rejects explicit values; GENERATED
        // BY DEFAULT ('d') and classic SERIAL (a nextval() default with no identity
        // clause) both accept them, so neither is treated as non-editable here.
        let rows = sqlx::query("SELECT a.attname::text AS name, pg_catalog.format_type(a.atttypid,a.atttypmod) AS type,
                NOT a.attnotnull AS nullable, pg_get_expr(d.adbin,d.adrelid) AS default_value,
                a.attidentity = 'a' AS identity,
                a.attgenerated <> '' AS generated,
                input.type AS input_type, input.record_input,
                (SELECT k.position::int FROM pg_index i, unnest(i.indkey) WITH ORDINALITY k(attnum,position)
                  WHERE i.indrelid=a.attrelid AND i.indisprimary AND k.position<=i.indnkeyatts AND k.attnum=a.attnum) AS pk_position
            FROM pg_attribute a LEFT JOIN pg_attrdef d ON d.adrelid=a.attrelid AND d.adnum=a.attnum
            LEFT JOIN LATERAL (
                WITH RECURSIVE chain(oid, depth) AS (
                    SELECT a.atttypid, 0
                    UNION ALL
                    SELECT CASE WHEN t.typtype='d' THEN t.typbasetype ELSE t.typelem END,
                           c.depth + CASE WHEN t.typtype='d' THEN 0 ELSE 1 END
                    FROM chain c JOIN pg_type t ON t.oid=c.oid
                    LEFT JOIN pg_type element ON element.oid=t.typelem
                    WHERE (t.typtype='d' AND NOT(c.depth>0 AND t.typcategory='A')) OR element.typarray=t.oid
                )
                SELECT format('%I.%I',n.nspname,t.typname) || repeat('[]',c.depth) AS type,
                       t.typtype='d' AS record_input
                FROM chain c JOIN pg_type t ON t.oid=c.oid
                JOIN pg_namespace n ON n.oid=t.typnamespace
                WHERE (t.typtype<>'d' OR (c.depth>0 AND t.typcategory='A')) AND NOT EXISTS(SELECT 1 FROM pg_type e WHERE e.oid=t.typelem AND e.typarray=t.oid)
            ) input ON true
            WHERE a.attrelid=$1::regclass AND a.attnum>0 AND NOT a.attisdropped ORDER BY a.attnum")
            .bind(relation).fetch_all(&pool).await.map_err(|e| e.to_string())?;
        Ok(rows
            .iter()
            .map(|r| {
                let field_type: String = r.get("type");
                let pk: Option<i32> = r.get("pk_position");
                let generated: bool = r.get("generated");
                let identity: bool = r.get("identity");
                PgColumn {
                    input_type: r.get("input_type"),
                    record_input: r.get("record_input"),
                    structure: ColumnStructure {
                        field: r.get("name"),
                        value_kind: values::kind(&field_type),
                        field_type,
                        nullable: r.get("nullable"),
                        primary_key_position: pk.map(|p| p as usize),
                        is_identity: identity,
                        is_generated: generated,
                        key: if pk.is_some() {
                            "PRI".into()
                        } else {
                            String::new()
                        },
                        default_value: r.get("default_value"),
                        extra: if generated {
                            "generated".into()
                        } else if identity {
                            "identity".into()
                        } else {
                            String::new()
                        },
                    },
                }
            })
            .collect())
    }
}
