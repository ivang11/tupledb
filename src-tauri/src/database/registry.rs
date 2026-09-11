use super::driver::DatabaseDriver;
use super::drivers::mysql;
use super::drivers::postgresql;
use crate::connections::{DatabaseEngine, DatabaseSettings};
use serde::Serialize;
use std::sync::Arc;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriverDescriptor {
    pub engine: DatabaseEngine,
    pub label: &'static str,
    pub default_port: Option<u16>,
}

/// Only implemented adapters belong here. Configuration may describe future
/// engines without advertising them as usable to the frontend.
pub const AVAILABLE_DRIVERS: &[DriverDescriptor] = &[
    DriverDescriptor {
        engine: DatabaseEngine::MySql,
        label: "MySQL",
        default_port: Some(3306),
    },
    DriverDescriptor {
        engine: DatabaseEngine::PostgreSql,
        label: "PostgreSQL",
        default_port: Some(5432),
    },
];

pub fn ensure_available(engine: DatabaseEngine) -> Result<(), String> {
    if AVAILABLE_DRIVERS
        .iter()
        .any(|driver| driver.engine == engine)
    {
        Ok(())
    } else {
        Err(format!("Database engine {engine:?} is not implemented yet"))
    }
}

pub struct OpenedDatabase {
    pub driver: Arc<dyn DatabaseDriver>,
    pub server_version: String,
}

pub struct ConnectOptions<'a> {
    pub read_only: bool,
    pub endpoint: Option<(&'a str, u16)>,
    pub timeout_secs: u64,
    pub tunneled: bool,
}

pub async fn open(
    settings: &DatabaseSettings,
    options: ConnectOptions<'_>,
) -> Result<OpenedDatabase, String> {
    ensure_available(settings.engine())?;
    match settings {
        DatabaseSettings::MySql(settings) => mysql::connection::open(settings, options).await,
        DatabaseSettings::PostgreSql(settings) => {
            postgresql::connection::open(settings, options).await
        }
        _ => Err("Database adapter unavailable".into()),
    }
}
