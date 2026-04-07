use serde::{Deserialize, Serialize};
use serde_json::Value;
use async_trait::async_trait;
use crate::filters::FilterSet;

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
pub struct ImportResult {
    pub executed: usize,
    pub errors: Vec<String>,
}

// --------------------------------------------------------------------------
// DatabaseDriver trait
// --------------------------------------------------------------------------

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
    ) -> Result<QueryResult, String>;

    /// Fetches all rows from a table as parsed JSON values, used for exports.
    async fn get_all_rows(
        &self,
        database: &str,
        table: &str,
    ) -> Result<(Vec<ColumnInfo>, Vec<Value>), String>;

    async fn execute_query(
        &self,
        database: Option<&str>,
        sql: &str,
    ) -> Result<RawQueryResult, String>;

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

    async fn truncate_table(
        &self,
        database: &str,
        table: &str,
        disable_fk_checks: bool,
    ) -> Result<(), String>;

    // Bulk import: SETs the database context, disables FK checks, runs all
    // statements, re-enables FK checks, and returns one Result per statement.
    async fn execute_statements(
        &self,
        database: &str,
        statements: &[String],
    ) -> Vec<Result<(), String>>;
}
