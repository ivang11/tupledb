use crate::connections::Environment;
use keyring::Entry;
use uuid::Uuid;

const APP_NAME: &str = "com.db-viewer.app";

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
    entry.set_password(password)
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

pub fn is_query_safe(query: &str, environment: Environment, allow_writes: bool) -> Result<(), String> {
    if environment == Environment::Production && !allow_writes {
        let q = query.trim().to_uppercase();
        let allowed_keywords = ["SELECT", "SHOW", "DESCRIBE", "EXPLAIN"];

        let is_allowed = allowed_keywords.iter().any(|&keyword| q.starts_with(keyword));

        if !is_allowed {
            return Err("Operation blocked: Production environment is READ-ONLY. Enable write access in connection settings.".into());
        }
    }
    Ok(())
}
