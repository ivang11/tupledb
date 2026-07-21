use crate::filters::FilterSet;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

// --------------------------------------------------------------------------
// Shared data types (formerly split across mysql.rs and schema.rs)
// --------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RawQueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Value>,
    pub rows_affected: u64,
    pub is_select: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Value>,
    pub total_count: i64,
    pub total_count_is_estimate: bool,
    pub timings: Option<TableDataTimings>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TableDataTimings {
    pub count_ms: u64,
    pub select_ms: u64,
    pub total_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeysetPage {
    pub column: String,
    pub value: Value,
    pub direction: String,
}

#[derive(Debug, Deserialize)]
pub struct TableChange {
    pub column: String,
    pub value: Value,
}

#[derive(Debug, Deserialize)]
pub struct RowChange {
    pub pk_column: String,
    pub pk_value: Value,
    pub changes: Vec<TableChange>,
}

#[derive(Debug, Deserialize)]
pub struct RowDeletion {
    pub pk_column: String,
    pub pk_value: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Table {
    pub name: String,
    pub table_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColumnStructure {
    pub field: String,
    pub field_type: String,
    pub nullable: bool,
    pub key: String,
    pub default_value: Option<String>,
    pub extra: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ForeignKey {
    pub column: String,
    pub referenced_table: String,
    pub referenced_column: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TableIndex {
    pub key_name: String,
    pub non_unique: bool,
    pub column_name: String,
    pub seq_in_index: u64,
    pub index_type: String,
    pub nullable: bool,
    pub comment: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportMetrics {
    pub parsed_statements: usize,
    pub compacted_statements: usize,
    pub executed_batches: usize,
    pub sql_blocks: usize,
    pub read_ms: u64,
    pub process_ms: u64,
    pub execute_ms: u64,
    pub total_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ImportResult {
    pub executed: usize,
    pub errors: Vec<String>,
    pub metrics: ImportMetrics,
}

pub type QueryChunkCallback = Arc<dyn Fn(Option<Vec<ColumnInfo>>, Vec<Value>) + Send + Sync>;

// --------------------------------------------------------------------------
// DatabaseDriver trait
// --------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    // Schema
    async fn get_databases(&self) -> Result<Vec<String>, String>;
    async fn create_database(&self, name: &str) -> Result<(), String>;
    async fn drop_database(&self, name: &str) -> Result<(), String>;
    async fn get_tables(&self, database: &str) -> Result<Vec<Table>, String>;
    async fn get_table_structure(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<ColumnStructure>, String>;
    /// Returns the CREATE TABLE DDL string for the given table.
    async fn get_table_ddl(&self, database: &str, table: &str) -> Result<String, String>;
    /// Lists only BASE TABLE names (no views) — used for bulk export.
    async fn get_base_tables(&self, database: &str) -> Result<Vec<String>, String>;
    async fn get_foreign_keys(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<ForeignKey>, String>;
    async fn get_table_indexes(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<TableIndex>, String>;

    /// Returns the PK column names in order (from information_schema.statistics).
    async fn get_primary_key_columns(
        &self,
        database: &str,
        table: &str,
    ) -> Result<Vec<String>, String>;

    /// Returns the estimated row count from information_schema.TABLES (fast, may be stale).
    async fn get_estimated_row_count(&self, database: &str, table: &str) -> Result<i64, String>;

    // Data
    async fn get_table_data(
        &self,
        database: &str,
        table: &str,
        page: u32,
        page_size: u32,
        filters: Option<FilterSet>,
        sort_column: Option<String>,
        sort_desc: Option<bool>,
        exact_count: bool,
        keyset: Option<KeysetPage>,
    ) -> Result<QueryResult, String>;

    /// Fetches all rows from a table as parsed JSON values, used for exports.
    async fn get_all_rows(
        &self,
        database: &str,
        table: &str,
    ) -> Result<(Vec<ColumnInfo>, Vec<Value>), String>;

    /// Streams all rows from a table, sending `(Some(columns), row)` for the first row
    /// and `(None, row)` for subsequent ones. Used for streaming exports to disk.
    async fn stream_all_rows(
        &self,
        database: &str,
        table: &str,
        tx: tokio::sync::mpsc::Sender<(Option<Vec<ColumnInfo>>, Value)>,
    ) -> Result<(), String> {
        let (columns, rows) = self.get_all_rows(database, table).await?;
        for (i, row) in rows.into_iter().enumerate() {
            let col = if i == 0 { Some(columns.clone()) } else { None };
            if tx.send((col, row)).await.is_err() {
                break;
            }
        }
        Ok(())
    }

    async fn execute_query(
        &self,
        database: Option<&str>,
        sql: &str,
        query_id: Option<&str>,
        on_progress: Option<Arc<dyn Fn(u64) + Send + Sync>>,
        on_chunk: Option<QueryChunkCallback>,
    ) -> Result<RawQueryResult, String>;

    /// Returns the MySQL thread id of a currently-running query, if tracked.
    fn get_thread_id_for_query(&self, _query_id: &str) -> Option<u64> {
        None
    }

    /// Returns the MySQL thread id of a currently-running import batch, if tracked.
    fn get_thread_id_for_import(&self, _import_id: &str) -> Option<u64> {
        None
    }

    /// Sends KILL QUERY to the server for the given thread id.
    async fn kill_query(&self, _thread_id: u64) -> Result<(), String> {
        Err("Query cancellation not supported for this driver".to_string())
    }

    /// Terminates the server connection for the given thread id.
    async fn kill_connection(&self, _thread_id: u64) -> Result<(), String> {
        Err("Connection termination not supported for this driver".to_string())
    }

    // Mutations
    async fn apply_table_changes(
        &self,
        database: &str,
        table: &str,
        updates: Vec<RowChange>,
        deletions: Vec<RowDeletion>,
        disable_fk_checks: bool,
    ) -> Result<(), String>;

    async fn insert_row(
        &self,
        database: &str,
        table: &str,
        values: Vec<TableChange>,
        disable_fk_checks: bool,
    ) -> Result<(), String>;

    async fn drop_table(
        &self,
        database: &str,
        table: &str,
        disable_fk_checks: bool,
    ) -> Result<(), String>;

    async fn drop_tables(
        &self,
        database: &str,
        tables: &[String],
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        for table in tables {
            self.drop_table(database, table, disable_fk_checks).await?;
        }
        Ok(())
    }

    async fn truncate_table(
        &self,
        database: &str,
        table: &str,
        disable_fk_checks: bool,
    ) -> Result<(), String>;

    // Bulk import: SETs the database context, disables FK checks, runs all
    // statements, re-enables FK checks, and returns one Result per statement.
    async fn begin_import_session(&self, _database: &str, _import_id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn abort_import_session(&self, _import_id: &str) -> Result<(), String> {
        Ok(())
    }

    async fn finish_import_session(&self, _import_id: &str) -> Result<(), String> {
        Ok(())
    }

    fn get_import_batch_bytes(&self, _import_id: &str) -> Option<usize> {
        None
    }

    async fn execute_statements(
        &self,
        database: &str,
        statements: &[String],
        import_id: Option<&str>,
    ) -> Vec<Result<(), String>>;
}
