use super::*;

pub(crate) struct MySqlDialect;

impl crate::database::sql::SqlDialect for MySqlDialect {
    fn quote_table_for_export(&self, table: &TableRef) -> Result<String, String> {
        mysql_table_parts(table)?;
        quote_identifier(&table.name)
    }
    fn quote_identifier(&self, name: &str) -> Result<String, String> {
        quote_identifier(name)
    }
    fn literal(&self, value: &Value) -> String {
        sql_literal(value)
    }
    fn export_prologue(&self) -> &str {
        "SET FOREIGN_KEY_CHECKS=0;"
    }
    fn export_epilogue(&self) -> &str {
        "SET FOREIGN_KEY_CHECKS=1;"
    }
    fn begin_transaction(&self) -> &str {
        "START TRANSACTION;"
    }
}

pub(super) fn sql_literal(value: &Value) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(v) => {
            if *v {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }
        Value::Number(v) => v.to_string(),
        Value::String(v) => format!("'{}'", v.replace('\\', "\\\\").replace('\'', "\\'")),
        other => format!(
            "'{}'",
            other.to_string().replace('\\', "\\\\").replace('\'', "\\'")
        ),
    }
}

pub(super) fn quote_identifier(identifier: &str) -> Result<String, String> {
    let trimmed = identifier.trim();
    if trimmed.is_empty() {
        return Err("Identifier cannot be empty".to_string());
    }
    if identifier.chars().count() > 64 || identifier.contains('\0') {
        return Err("Identifier must contain at most 64 characters".to_string());
    }
    Ok(format!("`{}`", identifier.replace('`', "``")))
}

pub(super) fn validate_column_type(column_type: &str) -> Result<String, String> {
    const TYPES: &[&str] = &[
        "BIGINT",
        "BINARY",
        "BIT",
        "BLOB",
        "BOOL",
        "BOOLEAN",
        "CHAR",
        "DATE",
        "DATETIME",
        "DEC",
        "DECIMAL",
        "DOUBLE",
        "ENUM",
        "FIXED",
        "FLOAT",
        "GEOMETRY",
        "GEOMETRYCOLLECTION",
        "INT",
        "INTEGER",
        "JSON",
        "LINESTRING",
        "LONGBLOB",
        "LONGTEXT",
        "MEDIUMBLOB",
        "MEDIUMINT",
        "MEDIUMTEXT",
        "MULTILINESTRING",
        "MULTIPOINT",
        "MULTIPOLYGON",
        "NUMERIC",
        "POINT",
        "POLYGON",
        "REAL",
        "SET",
        "SMALLINT",
        "TEXT",
        "TIME",
        "TIMESTAMP",
        "TINYBLOB",
        "TINYINT",
        "TINYTEXT",
        "VARBINARY",
        "VARCHAR",
        "YEAR",
    ];

    let value = column_type.trim();
    if value.is_empty() || value.len() > 1_000 {
        return Err("Enter a valid column type".to_string());
    }
    if value.contains(';')
        || value.contains('`')
        || value.contains('\0')
        || value.contains("--")
        || value.contains("/*")
        || value.contains("*/")
        || value.contains('#')
    {
        return Err("Column type contains unsupported SQL syntax".to_string());
    }

    let base_end = value
        .find(|c: char| c == '(' || c.is_whitespace())
        .unwrap_or(value.len());
    let base = value[..base_end].to_ascii_uppercase();
    if !TYPES.contains(&base.as_str()) {
        return Err(format!("Unsupported column type: {}", &value[..base_end]));
    }

    let mut depth = 0usize;
    let mut quoted = false;
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if quoted && ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' {
            quoted = !quoted;
            continue;
        }
        if quoted {
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' if depth == 0 => return Err("Column type has unbalanced parentheses".to_string()),
            ')' => depth -= 1,
            ',' if depth == 0 => {
                return Err("Column type contains unsupported SQL syntax".to_string())
            }
            c if c.is_ascii_alphanumeric()
                || c.is_ascii_whitespace()
                || matches!(c, '_' | ',' | '.' | '+' | '-') => {}
            _ => return Err("Column type contains unsupported characters".to_string()),
        }
    }
    if quoted || depth != 0 {
        return Err("Column type has an unfinished quote or parenthesis".to_string());
    }

    let suffix_start = if let Some(open) = value.find('(') {
        let mut suffix_depth = 0usize;
        let mut end = None;
        let mut in_quote = false;
        let mut suffix_escaped = false;
        for (offset, ch) in value[open..].char_indices() {
            if suffix_escaped {
                suffix_escaped = false;
                continue;
            }
            if in_quote && ch == '\\' {
                suffix_escaped = true;
                continue;
            }
            if ch == '\'' {
                in_quote = !in_quote;
            } else if !in_quote && ch == '(' {
                suffix_depth += 1;
            } else if !in_quote && ch == ')' {
                suffix_depth -= 1;
                if suffix_depth == 0 {
                    end = Some(open + offset + ch.len_utf8());
                    break;
                }
            }
        }
        end.unwrap_or(value.len())
    } else {
        base_end
    };
    let modifiers = value[suffix_start..].trim();
    if !modifiers.is_empty()
        && !modifiers
            .split_whitespace()
            .all(|word| matches!(word.to_ascii_uppercase().as_str(), "UNSIGNED" | "ZEROFILL"))
    {
        return Err("Only UNSIGNED and ZEROFILL modifiers are supported".to_string());
    }

    Ok(value.to_string())
}

pub(super) fn quote_mysql_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
}

pub(super) fn column_type_supports_charset(column_type: &str) -> bool {
    let base_end = column_type
        .find(|c: char| c == '(' || c.is_whitespace())
        .unwrap_or(column_type.len());
    matches!(
        column_type[..base_end].to_ascii_uppercase().as_str(),
        "CHAR" | "VARCHAR" | "TINYTEXT" | "TEXT" | "MEDIUMTEXT" | "LONGTEXT" | "ENUM" | "SET"
    )
}

pub(super) fn append_keyset_predicate(
    where_clause: &str,
    keyset: &KeysetPage,
    descending_query: bool,
) -> String {
    let op = if descending_query { "<" } else { ">" };
    let predicate = format!("`{}` {} {}", keyset.column, op, sql_literal(&keyset.value));
    if where_clause.trim().is_empty() {
        format!(" WHERE {}", predicate)
    } else {
        format!("{} AND {}", where_clause, predicate)
    }
}
