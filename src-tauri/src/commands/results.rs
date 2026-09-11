use crate::database::driver::{ColumnInfo, QueryResult};
use serde_json::Value;

pub(super) fn compact_rows_for_ipc(columns: &[ColumnInfo], rows: Vec<Value>) -> Vec<Value> {
    rows.into_iter()
        .map(|row| match row {
            Value::Object(mut values) => Value::Array(
                columns
                    .iter()
                    .map(|column| values.remove(&column.name).unwrap_or(Value::Null))
                    .collect(),
            ),
            row => row,
        })
        .collect()
}

pub(super) fn compact_query_result_for_ipc(mut result: QueryResult) -> QueryResult {
    result.rows = compact_rows_for_ipc(&result.columns, result.rows);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn compacts_named_rows_for_the_webview_ipc_contract() {
        let columns = vec![
            ColumnInfo {
                name: "id".into(),
                type_name: "INT".into(),
            },
            ColumnInfo {
                name: "name".into(),
                type_name: "VARCHAR".into(),
            },
            ColumnInfo {
                name: "nullable".into(),
                type_name: "VARCHAR".into(),
            },
        ];
        let rows = vec![json!({ "name": "Ada", "id": 7, "nullable": null })];

        assert_eq!(
            compact_rows_for_ipc(&columns, rows),
            vec![json!([7, "Ada", null])]
        );
    }
}
