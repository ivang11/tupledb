use crate::database::types::*;
use serde_json::{Map, Value};
use sqlx::{Column, Row, TypeInfo};

pub fn kind(type_name: &str) -> ValueKind {
    let t = type_name.to_ascii_lowercase();
    if t.ends_with("[]") {
        return ValueKind::Other;
    }
    match t.as_str() {
        "bool" | "boolean" => ValueKind::Boolean,
        "int2" | "int4" | "int8" | "smallint" | "integer" | "bigint" | "oid" => ValueKind::Integer,
        "float4" | "float8" | "real" | "double precision" => ValueKind::Float,
        "json" | "jsonb" => ValueKind::Json,
        "bytea" => ValueKind::Binary,
        _ if t.starts_with("numeric") || t.starts_with("decimal") => ValueKind::Decimal,
        _ if t.starts_with("timestamp")
            || t.starts_with("time")
            || t == "date"
            || t == "interval" =>
        {
            ValueKind::DateTime
        }
        _ if t.contains("char") || t == "text" || t == "uuid" || t == "name" => ValueKind::Text,
        _ => ValueKind::Other,
    }
}

pub fn columns(row: &sqlx::postgres::PgRow) -> Vec<ColumnInfo> {
    row.columns()
        .iter()
        .map(|c| ColumnInfo {
            name: c.name().into(),
            type_name: c.type_info().name().into(),
        })
        .collect()
}

pub fn text_value(text: Option<String>, type_name: &str) -> Result<Value, String> {
    let Some(text) = text else {
        return Ok(Value::Null);
    };
    match kind(type_name) {
        ValueKind::Boolean => Ok(Value::Bool(text == "t" || text == "true")),
        ValueKind::Integer => {
            let value = text.parse::<i64>().map_err(|e| e.to_string())?;
            Ok(
                if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&value) {
                    Value::from(value)
                } else {
                    Value::String(text)
                },
            )
        }
        ValueKind::Float => Ok(text
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or(Value::String(text))),
        // Numeric, JSON, arrays, temporal and extension values retain the server's
        // text representation. In particular, nested JSON numbers are not rounded.
        _ => Ok(Value::String(text)),
    }
}

/// Only use for simple-protocol rows or explicit ::text projections.
pub fn parse_text_row(
    row: &sqlx::postgres::PgRow,
    columns: &[ColumnInfo],
    positional: bool,
) -> Result<Value, String> {
    let values = columns
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let text = row
                .try_get_unchecked::<Option<String>, _>(i)
                .map_err(|e| format!("Cannot decode column {}: {e}", c.name))?;
            text_value(text, &c.type_name)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if positional {
        Ok(Value::Array(values))
    } else {
        Ok(Value::Object(
            columns
                .iter()
                .zip(values)
                .map(|(c, v)| (c.name.clone(), v))
                .collect::<Map<_, _>>(),
        ))
    }
}
