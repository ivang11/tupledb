use tauri::State;
use uuid::Uuid;
use sqlx::{mysql::MySqlConnectOptions, mysql::MySqlSslMode};
use crate::connections::Connection;
use crate::state::AppState;
use crate::ssh::SshTunnel;

#[tauri::command]
pub async fn get_connections(state: State<'_, AppState>) -> Result<Vec<Connection>, String> {
    let connections = state.connections_config.read();
    // Strip passwords before sending to frontend
    Ok(connections.values().map(|c| {
        let mut c = c.clone();
        c.mysql.password = None;
        if let Some(ssh) = &mut c.ssh {
            match &mut ssh.auth {
                crate::connections::SshAuth::Password { password } => { *password = String::new(); }
                crate::connections::SshAuth::Key { passphrase, .. } => { *passphrase = None; }
            }
        }
        c
    }).collect())
}


#[tauri::command]
pub async fn add_connection(state: State<'_, AppState>, mut connection: Connection) -> Result<(), String> {
    println!("Saving connection: {} (Env: {:?})", connection.name, connection.environment);

    // If editing and a password field is empty, preserve the existing stored password
    {
        let existing = state.connections_config.read();
        if let Some(existing_conn) = existing.get(&connection.id) {
            if connection.mysql.password.as_deref().unwrap_or("").is_empty() {
                connection.mysql.password = existing_conn.mysql.password.clone();
                println!("  -> Preserving existing MySQL password");
            }
            if let (Some(new_ssh), Some(old_ssh)) = (&mut connection.ssh, &existing_conn.ssh) {
                match (&mut new_ssh.auth, &old_ssh.auth) {
                    (crate::connections::SshAuth::Password { password: new_pw }, crate::connections::SshAuth::Password { password: old_pw }) => {
                        if new_pw.is_empty() && !old_pw.is_empty() {
                            *new_pw = old_pw.clone();
                            println!("  -> Preserving existing SSH password");
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Store full connection (with passwords) in config
    let mut connections = state.connections_config.write();
    connections.insert(connection.id, connection);
    drop(connections);
    state.save()
}

#[tauri::command]
pub async fn remove_connection(state: State<'_, AppState>, id: Uuid) -> Result<(), String> {
    println!("Removing connection: {}", id);
    
    // 1. Clean up active session if exists
    let mut sessions = state.active_sessions.write();
    if let Some(session) = sessions.remove(&id) {
        if let Some(tunnel) = session.tunnel {
            tunnel.disconnect();
        }
    }
    drop(sessions);

    // 2. Remove from config
    let mut connections = state.connections_config.write();
    connections.remove(&id);
    drop(connections);

    // 3. Save config
    state.save()
}

#[tauri::command]
pub async fn connect(state: State<'_, AppState>, connection: Connection) -> Result<(), String> {
    println!("Connecting to {} ({})", connection.name, connection.mysql.host);

    // Always use stored passwords from config (frontend never has them)
    let connection = {
        let configs = state.connections_config.read();
        match configs.get(&connection.id) {
            Some(stored) => stored.clone(),
            None => connection, // fallback to what frontend sent (new connection not yet saved)
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
        .acquire_timeout(std::time::Duration::from_secs(30))
        .test_before_acquire(true)
        .connect_with(opts)
        .await
        .map_err(|e| {
            println!("  -> Connection failed: {}", e);
            format!("MySQL Connection failed: {}", e)
        })?;

    println!("  -> Connection successful!");
    let mut sessions = state.active_sessions.write();
    // ... rest of logic
    // Clean up existing session if any
    if let Some(old_session) = sessions.remove(&connection.id) {
        if let Some(old_tunnel) = old_session.tunnel {
            old_tunnel.disconnect();
        }
    }

    sessions.insert(connection.id, crate::state::ActiveConnection {
        pool,
        tunnel,
    });

    Ok(())
}

#[tauri::command]
pub async fn test_connection(connection: Connection) -> Result<String, String> {
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
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect_with(opts)
        .await
        .map_err(|e| format!("MySQL Connection failed: {}", e))?;

    // Ping to verify
    sqlx::query("SELECT 1")
        .fetch_one(&pool)
        .await
        .map_err(|e| format!("MySQL Ping failed: {}", e))?;

    Ok("Connected successfully".into())
}
