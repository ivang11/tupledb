pub use super::types::*;
use crate::filters::FilterSet;
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;

#[async_trait]
pub trait DatabaseDriver: CatalogDriver + QueryDriver + EditDriver + ImportDriver {
    fn dialect(&self) -> &dyn super::sql::SqlDialect;
    fn capabilities(&self) -> super::capabilities::DatabaseCapabilities;
    async fn close(&self);

    async fn cancel_query(&self, _query_id: &str) -> Result<(), String> {
        Err("Query cancellation is not supported by this driver".into())
    }

    async fn cancel_import(&self, _import_id: &str) -> Result<(), String> {
        Err("Import cancellation is not supported by this driver".into())
    }
}

#[allow(clippy::too_many_arguments)]
#[async_trait]
pub trait CatalogDriver: Send + Sync {
    // Schema
    async fn get_databases(&self) -> Result<Vec<String>, String>;
    async fn get_database_creation_options(&self) -> Result<DatabaseCreationOptions, String>;
    async fn create_database(
        &self,
        name: &str,
        character_set: Option<&str>,
        collation: Option<&str>,
    ) -> Result<(), String>;
    async fn drop_database(&self, name: &str) -> Result<(), String>;
    async fn get_tables(&self, database: &str) -> Result<Vec<Table>, String>;
    async fn get_table_structure(&self, target: &TableRef) -> Result<Vec<ColumnStructure>, String>;
    /// Returns the CREATE TABLE DDL string for the given table.
    async fn get_table_ddl(&self, target: &TableRef) -> Result<String, String>;
    /// Lists only base tables (no views), retaining namespace identity.
    async fn get_base_tables(&self, database: &str) -> Result<Vec<TableRef>, String> {
        Ok(self
            .get_tables(database)
            .await?
            .into_iter()
            .filter(|table| table.table_type == "BASE TABLE")
            .map(|table| table.reference)
            .collect())
    }
    async fn get_foreign_keys(&self, target: &TableRef) -> Result<Vec<ForeignKey>, String>;
    async fn get_table_indexes(&self, target: &TableRef) -> Result<Vec<TableIndex>, String>;

    /// Returns the PK column names in order.
    async fn get_primary_key_columns(&self, target: &TableRef) -> Result<Vec<String>, String>;

    /// Returns the estimated row count when available (fast, may be stale).
    async fn get_estimated_row_count(&self, target: &TableRef) -> Result<i64, String>;
}

#[allow(clippy::too_many_arguments)]
#[async_trait]
pub trait QueryDriver: Send + Sync {
    // Data
    async fn get_table_data(
        &self,
        target: &TableRef,
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
        target: &TableRef,
    ) -> Result<(Vec<ColumnInfo>, Vec<Value>), String>;

    /// Streams all rows with bounded buffering. Send `(Some(columns), row)` for
    /// the first row and `(None, row)` afterwards. No buffered fallback is provided:
    /// each adapter must implement actual streaming for large exports.
    async fn stream_all_rows(
        &self,
        target: &TableRef,
        tx: tokio::sync::mpsc::Sender<(Option<Vec<ColumnInfo>>, Value)>,
    ) -> Result<(), String>;

    async fn execute_query(
        &self,
        database: Option<&str>,
        sql: &str,
        query_id: Option<&str>,
        on_progress: Option<Arc<dyn Fn(u64) + Send + Sync>>,
        on_chunk: Option<QueryChunkCallback>,
        max_retained_cells: Option<usize>,
    ) -> Result<RawQueryResult, String>;
}

#[allow(clippy::too_many_arguments)]
#[async_trait]
pub trait EditDriver: Send + Sync {
    // Mutations
    async fn apply_table_changes(
        &self,
        target: &TableRef,
        updates: Vec<RowChange>,
        deletions: Vec<RowDeletion>,
        disable_fk_checks: bool,
    ) -> Result<(), String>;

    async fn insert_row(
        &self,
        target: &TableRef,
        values: Vec<TableChange>,
        disable_fk_checks: bool,
    ) -> Result<(), String>;

    /// Renames a column and/or changes its data type while preserving the
    /// rest of its definition (nullability, default, generated expression,
    /// collation, comment and extra attributes).
    async fn alter_table_column(
        &self,
        target: &TableRef,
        old_name: &str,
        new_name: &str,
        new_type: &str,
    ) -> Result<String, String>;

    async fn drop_table(&self, target: &TableRef, disable_fk_checks: bool) -> Result<(), String>;

    async fn drop_tables(
        &self,
        database: &str,
        tables: &[TableRef],
        disable_fk_checks: bool,
    ) -> Result<(), String> {
        for table in tables {
            if table.catalog != database {
                return Err("Mismatched catalog".into());
            }
            self.drop_table(table, disable_fk_checks).await?;
        }
        Ok(())
    }

    async fn truncate_table(
        &self,
        target: &TableRef,
        disable_fk_checks: bool,
    ) -> Result<(), String>;
}

#[async_trait]
pub trait ImportDriver: Send + Sync {
    fn import_parser(&self) -> Result<Box<dyn super::sql::SqlImportParser>, String> {
        Err("SQL import is not supported by this driver".into())
    }
    /// The adapter owns session state and transaction/constraint semantics.
    async fn begin_import_session(&self, database: &str, import_id: &str) -> Result<(), String>;
    async fn abort_import_session(&self, import_id: &str) -> Result<(), String>;
    async fn finish_import_session(&self, import_id: &str) -> Result<(), String>;

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
