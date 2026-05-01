use crate::connections::Connection;
use crate::mysql::MySqlDriver;
use crate::ssh::SshTunnel;
use crate::state::{ActiveConnection, AppState};
use sqlx::{mysql::MySqlConnectOptions, mysql::MySqlSslMode};
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub async fn get_connections(state: State<'_, AppState>) -> Result<Vec<Connection>, String> {
    let connections = state.connections_config.read();
    // Strip passwords before sending to frontend
    Ok(connections
        .values()
        .map(|c| {
            let mut c = c.clone();
            c.mysql.password = None;
            if let Some(ssh) = &mut c.ssh {
                match &mut ssh.auth {
                    crate::connections::SshAuth::Password { password } => {
                        *password = String::new();
                    }
                    crate::connections::SshAuth::Key { passphrase, .. } => {
                        *passphrase = None;
                    }
                }
            }
            c
        })
        .collect())
}

#[tauri::command]
pub async fn add_connection(
    state: State<'_, AppState>,
    mut connection: Connection,
) -> Result<(), String> {
    println!(
        "Saving connection: {} (Env: {:?})",
        connection.name, connection.environment
    );

    // If editing and a password field is empty, preserve the existing stored password
    {
        let existing = state.connections_config.read();
        if let Some(existing_conn) = existing.get(&connection.id) {
            if connection
                .mysql
                .password
                .as_deref()
                .unwrap_or("")
                .is_empty()
            {
                connection.mysql.password = existing_conn.mysql.password.clone();
                println!("  -> Preserving existing MySQL password");
            }
            if let (Some(new_ssh), Some(old_ssh)) = (&mut connection.ssh, &existing_conn.ssh) {
                match (&mut new_ssh.auth, &old_ssh.auth) {
                    (
                        crate::connections::SshAuth::Password { password: new_pw },
                        crate::connections::SshAuth::Password { password: old_pw },
                    ) => {
                        if new_pw.is_empty() && !old_pw.is_empty() {
                            *new_pw = old_pw.clone();
                            println!("  -> Preserving existing SSH password");
                        }
                    }
                    (
                        crate::connections::SshAuth::Key {
                            passphrase: new_pp, ..
                        },
                        crate::connections::SshAuth::Key {
                            passphrase: old_pp, ..
                        },
                    ) => {
                        if new_pp.as_deref().unwrap_or("").is_empty()
                            && old_pp.as_deref().map_or(false, |p| !p.is_empty())
                        {
                            *new_pp = old_pp.clone();
                            println!("  -> Preserving existing SSH key passphrase");
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    let mut connections = state.connections_config.write();
    connections.insert(connection.id, connection);
    drop(connections);
    state.save()
}

#[tauri::command]
pub async fn remove_connection(state: State<'_, AppState>, id: Uuid) -> Result<(), String> {
    println!("Removing connection: {}", id);

    // Clean up active session if exists
    let mut sessions = state.active_sessions.write();
    if let Some(session) = sessions.remove(&id) {
        if let Some(tunnel) = session.tunnel {
            tunnel.disconnect();
        }
    }
    drop(sessions);

    let mut connections = state.connections_config.write();
    connections.remove(&id);
    drop(connections);

    state.save()
}

#[tauri::command]
pub async fn connect(state: State<'_, AppState>, connection: Connection) -> Result<(), String> {
    println!(
        "Connecting to {} ({})",
        connection.name, connection.mysql.host
    );

    // Always use stored passwords from config (frontend never has them)
    let connection = {
        let configs = state.connections_config.read();
        match configs.get(&connection.id) {
            Some(stored) => stored.clone(),
            None => connection,
        }
    };

    let (host, port, tunnel) = if let Some(ssh_settings) = &connection.ssh {
        println!("  -> Opening SSH tunnel to {}", ssh_settings.host);
        let tunnel = SshTunnel::new(ssh_settings, &connection.mysql.host, connection.mysql.port)?;
        ("127.0.0.1".to_string(), tunnel.local_port, Some(tunnel))
    } else {
        (connection.mysql.host.clone(), connection.mysql.port, None)
    };

    let mut opts = MySqlConnectOptions::new()
        .host(&host)
        .port(port)
        .username(&connection.mysql.user)
        .ssl_mode(MySqlSslMode::Disabled);

    if let Some(pw) = &connection.mysql.password {
        opts = opts.password(pw);
    }

    println!("  -> Establishing MySQL connection pool...");
    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(
            connection.timeout_secs.unwrap_or(30),
        ))
        .test_before_acquire(true)
        .connect_with(opts)
        .await
        .map_err(|e| {
            println!("  -> Connection failed: {}", e);
            format!("MySQL Connection failed: {}", e)
        })?;

    println!("  -> Connection successful!");

    // Detect if the server enforces ONLY_FULL_GROUP_BY (MySQL 5.7+).
    // We use this to disable it per-session during export reads so that VIEWs
    // created without strict mode can be exported without errors.
    let no_group_by_check = sqlx::query_scalar::<_, String>("SELECT @@SESSION.sql_mode")
        .fetch_one(&pool)
        .await
        .map(|mode| mode.contains("ONLY_FULL_GROUP_BY"))
        .unwrap_or(false);
    println!("  -> ONLY_FULL_GROUP_BY active: {}", no_group_by_check);

    let driver = Arc::new(MySqlDriver::new(pool, no_group_by_check));

    let mut sessions = state.active_sessions.write();
    if let Some(old_session) = sessions.remove(&connection.id) {
        if let Some(old_tunnel) = old_session.tunnel {
            old_tunnel.disconnect();
        }
    }

    sessions.insert(connection.id, ActiveConnection { driver, tunnel });
    Ok(())
}

#[tauri::command]
pub async fn export_connections(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let connections = state.connections_config.read();
    let content = serde_json::to_string_pretty(&*connections)
        .map_err(|e| format!("Failed to serialize connections: {}", e))?;
    std::fs::write(&path, content)
        .map_err(|e| format!("Failed to write file: {}", e))?;
    Ok(())
}

#[tauri::command]
pub async fn import_connections(state: State<'_, AppState>, path: String) -> Result<usize, String> {
    let content = std::fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let imported: std::collections::HashMap<Uuid, Connection> = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid connections file: {}", e))?;
    let count = imported.len();
    let mut connections = state.connections_config.write();
    for (id, conn) in imported {
        connections.insert(id, conn);
    }
    drop(connections);
    state.save()?;
    Ok(count)
}

#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    mut connection: Connection,
) -> Result<String, String> {
    // If editing an existing connection with empty password, use the stored one
    {
        let configs = state.connections_config.read();
        if let Some(stored) = configs.get(&connection.id) {
            if connection
                .mysql
                .password
                .as_deref()
                .unwrap_or("")
                .is_empty()
            {
                connection.mysql.password = stored.mysql.password.clone();
            }
        }
    }

    let (host, port, _tunnel) = if let Some(ssh_settings) = &connection.ssh {
        let tunnel = SshTunnel::new(ssh_settings, &connection.mysql.host, connection.mysql.port)?;
        ("127.0.0.1".to_string(), tunnel.local_port, Some(tunnel))
    } else {
        (connection.mysql.host.clone(), connection.mysql.port, None)
    };

    let mut opts = MySqlConnectOptions::new()
        .host(&host)
        .port(port)
        .username(&connection.mysql.user)
        .ssl_mode(MySqlSslMode::Disabled);

    if let Some(pw) = &connection.mysql.password {
        opts = opts.password(pw);
    }

    if let Some(db) = &connection.mysql.database {
        opts = opts.database(db);
    }

    let pool = sqlx::mysql::MySqlPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(
            connection.timeout_secs.unwrap_or(30),
        ))
        .connect_with(opts)
        .await
        .map_err(|e| format!("MySQL Connection failed: {}", e))?;

    sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("MySQL Ping failed: {}", e))?;

    Ok("Connected successfully".into())
}
