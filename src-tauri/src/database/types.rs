use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

/// An object identity, never a dotted SQL string. Each adapter decides which
/// parts to qualify (MySQL: catalog.name, PostgreSQL: schema.name).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct TableRef {
    pub catalog: String,
    pub schema: Option<String>,
    pub name: String,
}

impl std::fmt::Display for TableRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(schema) = &self.schema {
            write!(f, "{}.{}", schema, self.name)
        } else {
            write!(f, "{}", self.name)
        }
    }
}

impl TableRef {
    pub fn new(catalog: &str, name: &str) -> Self {
        Self {
            catalog: catalog.into(),
            schema: None,
            name: name.into(),
        }
    }
}

/// Compatibility at the IPC boundary only; adapters always receive TableRef.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TableTarget {
    Qualified(TableRef),
    LegacyName(String),
}

impl TableTarget {
    pub fn resolve(self, catalog: &str) -> Result<TableRef, String> {
        let target = match self {
            Self::Qualified(target) => target,
            Self::LegacyName(name) => TableRef::new(catalog, &name),
        };
        if target.catalog != catalog
            || target.name.is_empty()
            || target.schema.as_deref() == Some("")
        {
            return Err("Invalid table reference or mismatched catalog".into());
        }
        Ok(target)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ValueKind {
    Text,
    Integer,
    Decimal,
    Float,
    Boolean,
    DateTime,
    Json,
    Binary,
    Other,
}

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
    pub key: Vec<TableChange>,
    pub changes: Vec<TableChange>,
}

#[derive(Debug, Deserialize)]
pub struct RowDeletion {
    pub key: Vec<TableChange>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Table {
    pub reference: TableRef,
    pub name: String,
    pub table_type: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ColumnStructure {
    /// One-based position in the primary key, independent of column order.
    pub primary_key_position: Option<usize>,
    pub value_kind: ValueKind,
    pub is_identity: bool,
    pub is_generated: bool,
    pub field: String,
    pub field_type: String,
    pub nullable: bool,
    pub key: String,
    pub default_value: Option<String>,
    pub extra: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ForeignKey {
    pub constraint_name: String,
    pub position: usize,
    pub referenced: TableRef,
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

pub struct SqlExportOptions {
    pub mode: String,
    pub drop_if_exists: bool,
    pub use_transactions: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseCollation {
    pub name: String,
    pub character_set: String,
    pub is_default: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseCreationOptions {
    pub default_character_set: String,
    pub default_collation: String,
    pub collations: Vec<DatabaseCollation>,
}

pub type QueryChunkCallback = Arc<dyn Fn(Option<Vec<ColumnInfo>>, Vec<Value>) + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn qualified_targets_preserve_schema_and_reject_catalog_mismatch() {
        let target: TableTarget = serde_json::from_value(json!({
            "catalog": "app", "schema": "sales", "name": "a.b"
        }))
        .unwrap();
        assert_eq!(
            target.resolve("app").unwrap(),
            TableRef {
                catalog: "app".into(),
                schema: Some("sales".into()),
                name: "a.b".into()
            }
        );
        assert!(TableTarget::Qualified(TableRef::new("other", "users"))
            .resolve("app")
            .is_err());
        assert!(TableTarget::LegacyName("".into()).resolve("app").is_err());
    }

    #[test]
    fn legacy_names_are_only_adapted_at_the_ipc_boundary() {
        let target: TableTarget = serde_json::from_value(json!("users")).unwrap();
        assert_eq!(
            target.resolve("app").unwrap(),
            TableRef::new("app", "users")
        );
    }

    #[test]
    fn row_mutations_require_the_new_complete_key_contract() {
        assert!(
            serde_json::from_value::<RowDeletion>(json!({"pk_column": "id", "pk_value": 1}))
                .is_err()
        );
        let change: RowChange = serde_json::from_value(json!({
            "key": [{"column": "tenant", "value": "01"}, {"column": "id", "value": "18446744073709551615"}],
            "changes": [{"column": "note", "value": "NULL"}]
        })).unwrap();
        assert_eq!(change.key[1].value, json!("18446744073709551615"));
    }
}
