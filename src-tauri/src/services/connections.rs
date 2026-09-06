use crate::connections::Connection;
use crate::database::capabilities::ConnectionInfo;
use crate::database::registry::{self, ConnectOptions};
use crate::ssh::SshTunnel;
use crate::state::ActiveConnection;

/// Shared lifecycle for opening and testing a connection. Availability is
/// checked before creating any network connection or SSH process.
pub async fn open(connection: &Connection) -> Result<(ActiveConnection, ConnectionInfo), String> {
    registry::ensure_available(connection.database.engine())?;
    let endpoint = connection.database.network_endpoint();
    let tunnel = match (&connection.ssh, endpoint) {
        (Some(ssh), Some((host, port))) => Some(SshTunnel::new(ssh, host, port)?),
        (Some(_), None) => return Err("SSH requires a network database".into()),
        (None, _) => None,
    };
    let endpoint = tunnel
        .as_ref()
        .map(|t| ("127.0.0.1", t.local_port))
        .or(endpoint);
    let opened = registry::open(
        &connection.database,
        ConnectOptions {
            endpoint,
            timeout_secs: connection.timeout_secs.unwrap_or(30),
            tunneled: tunnel.is_some(),
        },
    )
    .await?;
    let info = ConnectionInfo {
        engine: connection.database.engine(),
        server_version: opened.server_version,
        capabilities: opened.driver.capabilities(),
    };
    Ok((
        ActiveConnection {
            driver: opened.driver,
            tunnel,
        },
        info,
    ))
}

pub async fn close(session: ActiveConnection) {
    session.driver.close().await;
    if let Some(tunnel) = session.tunnel {
        tunnel.disconnect();
    }
}
