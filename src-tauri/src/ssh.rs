use crate::connections::{SshAuth, SshSettings};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

struct AskpassFiles {
    script_path: PathBuf,
    secret_path: PathBuf,
}

impl AskpassFiles {
    fn new(secret: &str) -> Result<Self, String> {
        let id = uuid::Uuid::new_v4();
        let files = Self {
            script_path: std::env::temp_dir().join(format!(".tupledb_ssh_askpass_{id}.sh")),
            secret_path: std::env::temp_dir().join(format!(".tupledb_ssh_askpass_{id}.dat")),
        };

        std::fs::write(&files.secret_path, secret)
            .map_err(|e| format!("Failed to write SSH credential file: {e}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&files.secret_path, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| format!("Failed to protect SSH credential file: {e}"))?;
        }

        // Pass the data file path through the environment so paths never need shell escaping.
        std::fs::write(
            &files.script_path,
            "#!/bin/sh\ncat -- \"$TUPLEDB_SSH_SECRET_FILE\"\n",
        )
        .map_err(|e| format!("Failed to write SSH askpass helper: {e}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&files.script_path, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| format!("Failed to make SSH askpass helper executable: {e}"))?;
        }

        Ok(files)
    }

    fn configure(&self, command: &mut Command) {
        command
            .env("SSH_ASKPASS", &self.script_path)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("TUPLEDB_SSH_SECRET_FILE", &self.secret_path)
            // DISPLAY is needed on older OpenSSH versions to trigger askpass.
            .env(
                "DISPLAY",
                std::env::var("DISPLAY").unwrap_or_else(|_| ":0".into()),
            );
    }
}

impl Drop for AskpassFiles {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.script_path);
        let _ = std::fs::remove_file(&self.secret_path);
    }
}

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

        let mut askpass_files: Option<AskpassFiles> = None;

        let mut child = match &settings.auth {
            SshAuth::Key {
                private_key_path,
                passphrase,
            } => {
                let has_passphrase = passphrase.as_ref().is_some_and(|p| !p.is_empty());

                let mut cmd = Command::new("ssh");
                cmd.args(["-N", "-L", &forward])
                    .args(["-i", private_key_path])
                    .args(["-p", &settings.port.to_string()])
                    .args(["-o", "StrictHostKeyChecking=accept-new"])
                    .args(["-o", "ExitOnForwardFailure=yes"])
                    .args(["-o", "ServerAliveInterval=30"])
                    .args(["-o", "ServerAliveCountMax=3"]);

                if has_passphrase {
                    let files = AskpassFiles::new(passphrase.as_deref().unwrap())?;
                    files.configure(&mut cmd);
                    askpass_files = Some(files);
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
                let files = AskpassFiles::new(password)?;
                let mut cmd = Command::new("ssh");
                cmd.args(["-N", "-L", &forward])
                    .args(["-p", &settings.port.to_string()])
                    .args(["-o", "StrictHostKeyChecking=accept-new"])
                    .args(["-o", "ExitOnForwardFailure=yes"])
                    .args(["-o", "ServerAliveInterval=30"])
                    .args(["-o", "ServerAliveCountMax=3"])
                    .args([
                        "-o",
                        "PreferredAuthentications=password,keyboard-interactive",
                    ])
                    .args(["-o", "NumberOfPasswordPrompts=1"])
                    .arg(&dest)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::piped());

                files.configure(&mut cmd);

                // Detach from the controlling terminal so SSH always uses SSH_ASKPASS.
                #[cfg(unix)]
                unsafe {
                    use std::os::unix::process::CommandExt;
                    cmd.pre_exec(|| {
                        libc::setsid();
                        Ok(())
                    });
                }

                let child = cmd
                    .spawn()
                    .map_err(|e| format!("Failed to start ssh: {e}"))?;
                askpass_files = Some(files);
                child
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

        // SSH has completed authentication, so its temporary credential files can go away.
        drop(askpass_files.take());

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
