use crate::connections::DatabaseEngine;
use serde::{Deserialize, Serialize};

pub fn require(supported: bool, operation: &str) -> Result<(), String> {
    if supported {
        Ok(())
    } else {
        Err(format!(
            "{operation} is not supported by this database adapter"
        ))
    }
}

/// Features offered by the connected adapter. These describe implementation
/// support; server permissions and the connection's write policy still apply.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseCapabilities {
    pub schemas: bool,
    pub create_database: bool,
    pub database_collations: bool,
    pub edit_rows: bool,
    pub alter_columns: bool,
    pub truncate_table: bool,
    pub disable_foreign_key_checks: bool,
    pub cancel_query: bool,
    pub import_sql: bool,
    pub export_sql: bool,
    pub estimated_row_count: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionInfo {
    pub engine: DatabaseEngine,
    pub server_version: String,
    pub capabilities: DatabaseCapabilities,
}
