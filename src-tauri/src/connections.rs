use serde::{Deserialize, Serialize};
use uuid::Uuid;

fn default_allow_writes() -> bool {
    true
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(try_from = "StoredConnection")]
pub struct Connection {
    pub id: Uuid,
    pub name: String,
    pub environment: Environment,
    pub database: DatabaseSettings,
    pub ssh: Option<SshSettings>,
    #[serde(default)]
    pub timeout_secs: Option<u64>,
    #[serde(default = "default_allow_writes")]
    pub allow_writes: bool,
}

impl Connection {
    pub fn preserve_secrets_from(&mut self, stored: &Self) {
        self.database.preserve_password_from(&stored.database);
        if let (Some(new), Some(old)) = (&mut self.ssh, &stored.ssh) {
            match (&mut new.auth, &old.auth) {
                (SshAuth::Password { password: new }, SshAuth::Password { password: old })
                    if new.is_empty() =>
                {
                    *new = old.clone();
                }
                (
                    SshAuth::Key {
                        passphrase: new, ..
                    },
                    SshAuth::Key {
                        passphrase: old, ..
                    },
                ) if new.as_deref().unwrap_or("").is_empty() => {
                    *new = old.clone();
                }
                _ => {}
            }
        }
    }
}

/// Read both the original MySQL-only format and the tagged format. Serialization
/// always writes the tagged format. ConnectionStore migrates into a separate
/// versioned file and leaves the original file untouched for older versions.
#[derive(Deserialize)]
struct StoredConnection {
    id: Uuid,
    name: String,
    environment: Environment,
    database: Option<DatabaseSettings>,
    mysql: Option<MySqlSettings>,
    ssh: Option<SshSettings>,
    timeout_secs: Option<u64>,
    #[serde(default = "default_allow_writes")]
    allow_writes: bool,
}

impl TryFrom<StoredConnection> for Connection {
    type Error = String;

    fn try_from(stored: StoredConnection) -> Result<Self, Self::Error> {
        let database = match (stored.database, stored.mysql) {
            (Some(settings), None) => settings,
            (None, Some(settings)) => DatabaseSettings::MySql(settings),
            (Some(_), Some(_)) => {
                return Err("Ambiguous connection: both database and mysql settings".into())
            }
            (None, None) => return Err("Missing database settings".into()),
        };
        Ok(Self {
            id: stored.id,
            name: stored.name,
            environment: stored.environment,
            database,
            ssh: stored.ssh,
            timeout_secs: stored.timeout_secs,
            allow_writes: stored.allow_writes,
        })
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseEngine {
    #[serde(rename = "mysql")]
    MySql,
    #[serde(rename = "postgresql")]
    PostgreSql,
    #[serde(rename = "sqlite")]
    Sqlite,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "engine", content = "settings")]
pub enum DatabaseSettings {
    #[serde(rename = "mysql")]
    MySql(MySqlSettings),
    #[serde(rename = "postgresql")]
    PostgreSql(PostgreSqlSettings),
    #[serde(rename = "sqlite")]
    Sqlite(SqliteSettings),
}

impl DatabaseSettings {
    pub fn engine(&self) -> DatabaseEngine {
        match self {
            Self::MySql(_) => DatabaseEngine::MySql,
            Self::PostgreSql(_) => DatabaseEngine::PostgreSql,
            Self::Sqlite(_) => DatabaseEngine::Sqlite,
        }
    }

    pub fn network_endpoint(&self) -> Option<(&str, u16)> {
        match self {
            Self::MySql(s) => Some((&s.host, s.port)),
            Self::PostgreSql(s) => Some((&s.host, s.port)),
            Self::Sqlite(_) => None,
        }
    }

    pub fn configured_database(&self) -> Option<&str> {
        match self {
            Self::MySql(s) => s.database.as_deref().filter(|s| !s.is_empty()),
            Self::PostgreSql(s) => s.database.as_deref().filter(|s| !s.is_empty()),
            Self::Sqlite(_) => None,
        }
    }

    fn password(&self) -> Option<&String> {
        match self {
            Self::MySql(s) => s.password.as_ref(),
            Self::PostgreSql(s) => s.password.as_ref(),
            Self::Sqlite(_) => None,
        }
    }

    fn password_mut(&mut self) -> Option<&mut Option<String>> {
        match self {
            Self::MySql(s) => Some(&mut s.password),
            Self::PostgreSql(s) => Some(&mut s.password),
            Self::Sqlite(_) => None,
        }
    }

    pub fn strip_password(&mut self) {
        if let Some(password) = self.password_mut() {
            *password = None;
        }
    }

    pub fn preserve_password_from(&mut self, stored: &Self) {
        // Credentials must never migrate from one engine to another.
        if self.engine() == stored.engine() {
            if let Some(password) = self.password_mut() {
                if password.as_deref().unwrap_or("").is_empty() {
                    *password = stored.password().cloned();
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PostgreSqlSettings {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    #[serde(default)]
    pub database: Option<String>,
    pub ssl_mode: PostgreSqlSslMode,
    /// PEM-encoded CA certificate used to validate the server under
    /// `verify_ca`/`verify_full`. Not a secret: it is the public certificate
    /// that signed the server's certificate, not a private key.
    #[serde(default)]
    pub ssl_root_cert: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum PostgreSqlSslMode {
    Disable,
    Prefer,
    Require,
    VerifyCa,
    VerifyFull,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SqliteSettings {
    pub path: String,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Environment {
    Local,
    Dev,
    Staging,
    Production,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct MySqlSettings {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub database: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SshSettings {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub auth: SshAuth,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SshAuth {
    Password {
        password: String,
    },
    Key {
        private_key_path: String,
        passphrase: Option<String>,
    },
}
