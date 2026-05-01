use crate::connections::{SshAuth, SshSettings};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

pub struct SshTunnel {
    pub local_port: u16,
    child: Child,
}

impl SshTunnel {
    pub fn new(
        settings: &SshSettings,
        remote_host: &str,
        remote_port: u16,
    ) -> Result<Self, String> {
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

        let mut askpass_script: Option<std::path::PathBuf> = None;

        let mut child = match &settings.auth {
            SshAuth::Key {
                private_key_path,
                passphrase,
            } => {
                let has_passphrase = passphrase.as_ref().map_or(false, |p| !p.is_empty());

                let mut cmd = Command::new("ssh");
                cmd.args(["-N", "-L", &forward])
                    .args(["-i", private_key_path])
                    .args(["-p", &settings.port.to_string()])
                    .args(["-o", "StrictHostKeyChecking=accept-new"])
                    .args(["-o", "ExitOnForwardFailure=yes"])
                    .args(["-o", "ServerAliveInterval=30"])
                    .args(["-o", "ServerAliveCountMax=3"]);

                if has_passphrase {
                    // Write a temporary askpass script that echoes the passphrase.
                    // SSH_ASKPASS_REQUIRE=force tells OpenSSH to use it instead of the TTY.
                    let script_path = std::env::temp_dir()
                        .join(format!(".ssh_askpass_{}.sh", std::process::id()));

                    // Write the passphrase to a separate data file to avoid any shell escaping issues
                    let data_path = std::env::temp_dir()
                        .join(format!(".ssh_askpass_{}.dat", std::process::id()));

                    std::fs::write(&data_path, passphrase.as_deref().unwrap())
                        .map_err(|e| format!("Failed to write passphrase file: {}", e))?;

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(
                            &data_path,
                            std::fs::Permissions::from_mode(0o600),
                        )
                        .map_err(|e| format!("Failed to set passphrase file permissions: {}", e))?;
                    }

                    let script_content = format!("#!/bin/sh\ncat '{}'\n", data_path.display());
                    std::fs::write(&script_path, &script_content)
                        .map_err(|e| format!("Failed to write askpass script: {}", e))?;

                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        std::fs::set_permissions(
                            &script_path,
                            std::fs::Permissions::from_mode(0o700),
                        )
                        .map_err(|e| format!("Failed to set askpass script permissions: {}", e))?;
                    }

                    cmd.env("SSH_ASKPASS", &script_path)
                        .env("SSH_ASKPASS_REQUIRE", "force")
                        // DISPLAY is needed on older OpenSSH versions to trigger askpass
                        .env(
                            "DISPLAY",
                            std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into()),
                        );

                    askpass_script = Some(script_path);
                } else {
                    cmd.args(["-o", "BatchMode=yes"]);
                }

                cmd.arg(&dest)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped());

                // Detach from the controlling terminal so SSH uses SSH_ASKPASS
                // instead of trying to read the passphrase from /dev/tty.
                #[cfg(unix)]
                unsafe {
                    use std::os::unix::process::CommandExt;
                    cmd.pre_exec(|| {
                        libc::setsid();
                        Ok(())
                    });
                }

                cmd.spawn()
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
                    return Err("SSH password authentication requires 'sshpass'.\n\
                         Install it with: sudo apt install sshpass\n\
                         Or use key-based authentication instead."
                        .into());
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

        // Clean up temp askpass files now that SSH has read the passphrase
        if let Some(ref script_path) = askpass_script {
            let data_path = script_path.with_extension("dat");
            let _ = std::fs::remove_file(script_path);
            let _ = std::fs::remove_file(data_path);
        }

        if !ready {
            // Collect SSH stderr to surface the real error
            let stderr_msg = child
                .stderr
                .take()
                .and_then(|mut s| {
                    use std::io::Read;
                    let mut buf = String::new();
                    s.read_to_string(&mut buf).ok()?;
                    if buf.trim().is_empty() {
                        None
                    } else {
                        Some(buf.trim().to_string())
                    }
                })
                .unwrap_or_default();

            let _ = child.kill();

            return Err(if stderr_msg.is_empty() {
                "SSH tunnel did not become ready in time.\nCheck SSH credentials and server connectivity.".into()
            } else {
                format!("SSH tunnel failed: {}", stderr_msg)
            });
        }

        println!("SSH: Tunnel ready on 127.0.0.1:{}", local_port);

        Ok(Self { local_port, child })
    }

    pub fn disconnect(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
