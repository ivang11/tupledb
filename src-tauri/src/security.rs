use crate::connections::Environment;
use keyring::Entry;
use uuid::Uuid;

const APP_NAME: &str = "tupledb";

pub enum SecretType {
    MySql,
    Ssh,
}

pub fn set_secret(conn_id: Uuid, secret_type: SecretType, password: &str) -> Result<(), String> {
    let type_suffix = match secret_type {
        SecretType::MySql => "mysql",
        SecretType::Ssh => "ssh",
    };
    let entry = Entry::new(APP_NAME, &format!("{}-{}", conn_id, type_suffix))
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
    entry
        .set_password(password)
        .map_err(|e| format!("Failed to set password in keyring: {}", e))
}

pub fn get_secret(conn_id: Uuid, secret_type: SecretType) -> Result<Option<String>, String> {
    let type_suffix = match secret_type {
        SecretType::MySql => "mysql",
        SecretType::Ssh => "ssh",
    };
    let entry = Entry::new(APP_NAME, &format!("{}-{}", conn_id, type_suffix))
        .map_err(|e| format!("Failed to create keyring entry: {}", e))?;
    match entry.get_password() {
        Ok(pw) => Ok(Some(pw)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("Failed to get password from keyring: {}", e)),
    }
}

/// Valid identifier for ORDER BY (avoids injection; matches typical MySQL column names).
pub fn is_safe_sort_column(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$' || c == '-')
}

pub fn is_query_safe(
    query: &str,
    _environment: Environment,
    allow_writes: bool,
) -> Result<(), String> {
    if !allow_writes {
        let q = query.trim().to_uppercase();
        let allowed_keywords = ["SELECT", "SHOW", "DESCRIBE", "EXPLAIN"];

        let is_allowed = allowed_keywords
            .iter()
            .any(|&keyword| q.starts_with(keyword));

        if !is_allowed {
            return Err(read_only_error());
        }
    }
    Ok(())
}

pub fn ensure_writes_allowed(allow_writes: bool) -> Result<(), String> {
    if allow_writes {
        Ok(())
    } else {
        Err(read_only_error())
    }
}

fn read_only_error() -> String {
    "Operation blocked: Connection is READ-ONLY. Disable read-only mode in connection settings to allow writes.".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_safe_sort_columns() {
        for column in ["id", "user_name", "price$", "legacy-id", "A123"] {
            assert!(is_safe_sort_column(column), "{column} should be accepted");
        }
    }

    #[test]
    fn rejects_unsafe_sort_columns() {
        let too_long = "a".repeat(65);
        for column in [
            "",
            "user name",
            "name` DESC",
            "name;DROP",
            "schema.table",
            &too_long,
        ] {
            assert!(!is_safe_sort_column(column), "{column} should be rejected");
        }
    }

    #[test]
    fn allows_read_queries_in_read_only_mode() {
        for query in [
            "SELECT * FROM users",
            " show tables",
            "DESCRIBE users",
            "\nexplain select * from users",
        ] {
            assert!(is_query_safe(query, Environment::Production, false).is_ok());
        }
    }

    #[test]
    fn blocks_write_queries_in_read_only_mode() {
        for query in [
            "INSERT INTO users(id) VALUES (1)",
            "update users set name = 'Ada'",
            "DELETE FROM users",
            "DROP TABLE users",
            "TRUNCATE TABLE users",
        ] {
            assert!(is_query_safe(query, Environment::Local, false).is_err());
            assert!(is_query_safe(query, Environment::Production, false).is_err());
        }
    }

    #[test]
    fn allows_writes_when_explicitly_enabled() {
        assert!(is_query_safe("DROP TABLE users", Environment::Local, true).is_ok());
        assert!(is_query_safe("DROP TABLE users", Environment::Dev, true).is_ok());
        assert!(is_query_safe("DROP TABLE users", Environment::Production, true).is_ok());
    }

    #[test]
    fn blocks_non_query_writes_in_read_only_mode() {
        assert!(ensure_writes_allowed(false).is_err());
        assert!(ensure_writes_allowed(true).is_ok());
    }
}
