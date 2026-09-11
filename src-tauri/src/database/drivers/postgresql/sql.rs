use crate::database::{sql::SqlDialect, types::*};
use serde_json::Value;

pub fn quote_identifier(name: &str) -> Result<String, String> {
    if name.is_empty() || name.len() > 63 || name.contains('\0') {
        return Err("Invalid PostgreSQL identifier (maximum 63 bytes)".into());
    }
    Ok(format!("\"{}\"", name.replace('"', "\"\"")))
}

pub fn relation(table: &TableRef) -> Result<String, String> {
    let schema = table
        .schema
        .as_deref()
        .ok_or("PostgreSQL table operations require an explicit schema")?;
    Ok(format!(
        "{}.{}",
        quote_identifier(schema)?,
        quote_identifier(&table.name)?
    ))
}

pub struct PostgreSqlDialect;
impl SqlDialect for PostgreSqlDialect {
    fn quote_identifier(&self, name: &str) -> Result<String, String> {
        quote_identifier(name)
    }
    fn quote_table_for_export(&self, table: &TableRef) -> Result<String, String> {
        relation(table)
    }
    fn literal(&self, value: &Value) -> String {
        match value {
            Value::Null => "NULL".into(),
            Value::Bool(v) => {
                if *v {
                    "TRUE".into()
                } else {
                    "FALSE".into()
                }
            }
            Value::Number(v) => v.to_string(),
            Value::String(v) => format!("E'{}'", v.replace('\\', "\\\\").replace('\'', "''")),
            v => self.literal(&Value::String(v.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quotes_names_without_flattening_namespaces() {
        let table = TableRef {
            catalog: "app".into(),
            schema: Some("a.b".into()),
            name: "c\"d".into(),
        };
        assert_eq!(relation(&table).unwrap(), "\"a.b\".\"c\"\"d\"");
        assert!(relation(&TableRef::new("app", "users")).is_err());
        assert!(quote_identifier(&"ñ".repeat(32)).is_err());
        assert!(quote_identifier("bad\0name").is_err());
    }
}
