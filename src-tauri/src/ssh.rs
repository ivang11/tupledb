use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;
use crate::connections::{SshSettings, SshAuth};

pub struct SshTunnel {
    pub local_port: u16,
    child: Child,
}

impl SshTunnel {
    pub fn new(settings: &SshSettings, remote_host: &str, remote_port: u16) -> Result<Self, String> {
        // Find a free local port
        let local_port = {
            let listener = TcpListener::bind("127.0.0.1:0")
                .map_err(|e| format!("Cannot bind local port: {}", e))?;
            listener.local_addr().unwrap().port()
            // listener drops here, releasing the port for ssh to bind
        };

        println!(
            "SSH: Starting tunnel 127.0.0.1:{} -> {}:{} via {}@{}:{}",
            local_port, remote_host, remote_port, settings.user, settings.host, settings.port
        );

        let forward = format!("{}:{}:{}", local_port, remote_host, remote_port);
        let dest = format!("{}@{}", settings.user, settings.host);

        let child = match &settings.auth {
            SshAuth::Key { private_key_path, passphrase } => {
                let mut cmd = Command::new("ssh");
                cmd.args(["-N", "-L", &forward])
                    .args(["-i", private_key_path])
                    .args(["-p", &settings.port.to_string()])
                    .args(["-o", "StrictHostKeyChecking=accept-new"])
                    .args(["-o", "ExitOnForwardFailure=yes"])
                    .args(["-o", "ServerAliveInterval=30"])
                    .args(["-o", "ServerAliveCountMax=3"]);

                // Only use BatchMode (disable password prompt) when there's no passphrase
                let has_passphrase = passphrase.as_ref().map_or(false, |p| !p.is_empty());
                if !has_passphrase {
                    cmd.args(["-o", "BatchMode=yes"]);
                }

                cmd.arg(&dest)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| format!("Failed to start ssh: {}", e))?
            }

            SshAuth::Password { password } => {
                // sshpass is required for password-based SSH
                let has_sshpass = Command::new("sh")
                    .args(["-c", "command -v sshpass"])
                    .output()
                    .map(|o| o.status.success())
                    .unwrap_or(false);

                if !has_sshpass {
                    return Err(
                        "SSH password authentication requires 'sshpass'.\n\
                         Install it with: sudo apt install sshpass\n\
                         Or use key-based authentication instead.".into()
                    );
                }

                Command::new("sshpass")
                    .args(["-p", password])
                    .arg("ssh")
                    .args(["-N", "-L", &forward])
                    .args(["-p", &settings.port.to_string()])
                    .args(["-o", "StrictHostKeyChecking=accept-new"])
                    .args(["-o", "ExitOnForwardFailure=yes"])
                    .args(["-o", "ServerAliveInterval=30"])
                    .args(["-o", "ServerAliveCountMax=3"])
                    .arg(&dest)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                    .map_err(|e| format!("Failed to start sshpass: {}", e))?
            }
        };

        // Wait up to 15 seconds for the tunnel port to become available
        let mut ready = false;
        for _ in 0..75 {
            thread::sleep(Duration::from_millis(200));
            if TcpStream::connect(format!("127.0.0.1:{}", local_port)).is_ok() {
                ready = true;
                break;
            }
        }

        if !ready {
            return Err(
                "SSH tunnel did not become ready in time.\n\
                 Check SSH credentials and server connectivity.".into()
            );
        }

        println!("SSH: Tunnel ready on 127.0.0.1:{}", local_port);

        Ok(Self { local_port, child })
    }

    pub fn disconnect(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
