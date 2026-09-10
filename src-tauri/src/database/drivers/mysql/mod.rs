mod catalog;
mod editing;
mod import;
mod queries;
pub mod query_builder;
pub(crate) mod script;
pub(crate) mod sql;
mod values;
use sql::*;
use values::*;
pub mod connection;
use crate::database::driver::*;
use crate::database::results::retained_row_limit;
use crate::filters::FilterSet;
use async_trait::async_trait;
use chrono::Timelike;
use futures::StreamExt;
use parking_lot::RwLock;
use serde_json::{Map, Value};
use sqlx::{Column, MySql, MySqlPool, Row, TypeInfo, ValueRef};
use std::collections::HashMap;
use std::sync::Arc;

fn mysql_table_parts(target: &TableRef) -> Result<(&str, &str), String> {
    if target.schema.is_some() {
        return Err("MySQL does not use a separate schema in table references".into());
    }
    quote_identifier(&target.catalog)?;
    quote_identifier(&target.name)?;
    Ok((&target.catalog, &target.name))
}

fn is_early_connection_close(error: &sqlx::Error) -> bool {
    let msg = error.to_string().to_lowercase();
    msg.contains("got 0 bytes at eof")
        || msg.contains("early eof")
        || msg.contains("connection reset")
        || msg.contains("connection closed")
}

// --------------------------------------------------------------------------
// MySqlDriver
// --------------------------------------------------------------------------

pub struct MySqlDriver {
    pool: MySqlPool,
    running_queries: Arc<RwLock<HashMap<String, u64>>>,
    running_imports: Arc<RwLock<HashMap<String, u64>>>,
    import_sessions: Arc<RwLock<HashMap<String, Arc<ImportSession>>>>,
    /// True when the server has ONLY_FULL_GROUP_BY enabled (MySQL 5.7+).
    /// Used to disable it for the session in read-only export queries so that
    /// VIEWs created without strict mode can still be read.
    no_group_by_check: bool,
}

struct ImportSession {
    conn: tokio::sync::Mutex<sqlx::pool::PoolConnection<MySql>>,
    max_batch_bytes: usize,
}

impl MySqlDriver {
    pub fn new(pool: MySqlPool, no_group_by_check: bool) -> Self {
        Self {
            pool,
            running_queries: Arc::new(RwLock::new(HashMap::new())),
            running_imports: Arc::new(RwLock::new(HashMap::new())),
            import_sessions: Arc::new(RwLock::new(HashMap::new())),
            no_group_by_check,
        }
    }
}

#[async_trait]
impl DatabaseDriver for MySqlDriver {
    fn dialect(&self) -> &dyn crate::database::sql::SqlDialect {
        &sql::MySqlDialect
    }
    fn capabilities(&self) -> crate::database::capabilities::DatabaseCapabilities {
        crate::database::capabilities::DatabaseCapabilities {
            schemas: false,
            create_database: true,
            database_collations: true,
            edit_rows: true,
            alter_columns: true,
            truncate_table: true,
            disable_foreign_key_checks: true,
            cancel_query: true,
            import_sql: true,
            export_sql: true,
            inspect_ddl: true,
            estimated_row_count: true,
        }
    }

    async fn close(&self) {
        self.import_sessions.write().clear();
        self.pool.close().await;
    }

    async fn cancel_query(&self, query_id: &str) -> Result<(), String> {
        if let Some(thread_id) = self.get_thread_id_for_query(query_id) {
            self.kill_query(thread_id).await?;
        }
        Ok(())
    }

    async fn cancel_import(&self, import_id: &str) -> Result<(), String> {
        let thread_id = self.get_thread_id_for_import(import_id);
        self.abort_import_session(import_id).await?;
        if let Some(thread_id) = thread_id {
            self.kill_connection(thread_id).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sql_literal_formats_json_values_for_keyset_cursor() {
        assert_eq!(sql_literal(&Value::Null), "NULL");
        assert_eq!(sql_literal(&Value::Bool(true)), "1");
        assert_eq!(sql_literal(&Value::Bool(false)), "0");
        assert_eq!(sql_literal(&json!(42)), "42");
        assert_eq!(
            sql_literal(&Value::String("O'Reilly".into())),
            "'O\\'Reilly'"
        );
        assert_eq!(sql_literal(&Value::String("C:\\tmp".into())), "'C:\\\\tmp'");
    }

    #[test]
    fn validates_column_types_used_by_the_structure_editor() {
        for value in [
            "varchar(255)",
            "BIGINT UNSIGNED",
            "decimal(10, 2)",
            "enum('draft','published')",
            "timestamp(6)",
        ] {
            assert_eq!(validate_column_type(value).unwrap(), value);
        }
    }

    #[test]
    fn rejects_column_type_sql_injection_and_definition_attributes() {
        for value in [
            "varchar(20); DROP TABLE users",
            "int, DROP COLUMN email",
            "int NOT NULL",
            "varchar(20) COMMENT 'surprise'",
            "made_up_type",
            "enum('unfinished)",
        ] {
            assert!(
                validate_column_type(value).is_err(),
                "{value} should be rejected"
            );
        }
    }

    #[test]
    fn quotes_mysql_identifiers_and_rejects_invalid_names() {
        assert_eq!(quote_identifier("display name").unwrap(), "`display name`");
        assert_eq!(quote_identifier("odd`name").unwrap(), "`odd``name`");
        assert!(quote_identifier("").is_err());
        assert!(quote_identifier(&"a".repeat(65)).is_err());
    }

    #[test]
    fn only_character_types_retain_charset_and_collation() {
        assert!(column_type_supports_charset("varchar(255)"));
        assert!(column_type_supports_charset("ENUM('a','b')"));
        assert!(!column_type_supports_charset("int unsigned"));
        assert!(!column_type_supports_charset("varbinary(32)"));
    }

    #[test]
    fn append_keyset_predicate_adds_where_or_and() {
        let keyset = KeysetPage {
            column: "id".to_string(),
            value: json!(100),
            direction: "next".to_string(),
        };

        assert_eq!(
            append_keyset_predicate("", &keyset, false),
            " WHERE `id` > 100"
        );
        assert_eq!(
            append_keyset_predicate(" WHERE `status` = ?", &keyset, false),
            " WHERE `status` = ? AND `id` > 100"
        );
        assert_eq!(
            append_keyset_predicate(" WHERE `status` = ?", &keyset, true),
            " WHERE `status` = ? AND `id` < 100"
        );
    }

    #[test]
    fn mysql_wkb_to_wkt_decodes_point_with_mysql_srid_prefix() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&4326u32.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&1u32.to_le_bytes());
        bytes.extend_from_slice(&1.5f64.to_le_bytes());
        bytes.extend_from_slice(&2.25f64.to_le_bytes());

        assert_eq!(mysql_wkb_to_wkt(&bytes), "POINT(1.5 2.25)");
    }

    #[test]
    fn mysql_wkb_to_wkt_falls_back_to_hex_for_unknown_geometry() {
        assert_eq!(mysql_wkb_to_wkt(&[0x01, 0x02, 0xab]), "0x0102ab");
    }
}

impl MySqlDriver {
    pub fn get_thread_id_for_query(&self, query_id: &str) -> Option<u64> {
        self.running_queries.read().get(query_id).copied()
    }

    pub fn get_thread_id_for_import(&self, import_id: &str) -> Option<u64> {
        self.running_imports.read().get(import_id).copied()
    }

    pub async fn kill_query(&self, thread_id: u64) -> Result<(), String> {
        let kill_sql = format!("KILL QUERY {}", thread_id);
        sqlx::query(&kill_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub async fn kill_connection(&self, thread_id: u64) -> Result<(), String> {
        let kill_sql = format!("KILL CONNECTION {}", thread_id);
        sqlx::query(&kill_sql)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
