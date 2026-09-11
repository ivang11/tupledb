//! Fail-closed, versioned connection persistence. Never rewrite the legacy file.
use crate::connections::Connection;
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub type Connections = HashMap<Uuid, Connection>;
const FILE_NAME: &str = "connections.v2.json";

pub fn development_profile() -> bool {
    tauri::is_dev() || cfg!(debug_assertions)
}

pub fn config_directory(base: PathBuf, development: bool) -> PathBuf {
    if development {
        base.join("development")
    } else {
        base
    }
}

pub struct ConnectionStore {
    directory: PathBuf,
    protected_root: PathBuf,
    data: Connections,
    snapshot: Option<Vec<u8>>,
    error: Option<String>,
}

struct StoreLock(File);

impl Drop for StoreLock {
    fn drop(&mut self) {
        // Explicitly unlock before closing: a concurrent process spawn can
        // briefly inherit the descriptor before close-on-exec takes effect.
        let _ = self.0.unlock();
    }
}

impl ConnectionStore {
    pub fn open(directory: PathBuf) -> Self {
        let mut store = Self {
            protected_root: directory.clone(),
            directory,
            data: HashMap::new(),
            snapshot: None,
            error: None,
        };
        let _ = store.reload(); // Retain the error; callers cannot read/write an empty fallback.
        store
    }

    pub fn open_profile(base: PathBuf, development: bool) -> Self {
        let mut store = Self::open(config_directory(base.clone(), development));
        store.protected_root = base;
        store
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn data(&self) -> Result<&Connections, String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        Ok(&self.data)
    }

    pub fn reload(&mut self) -> Result<(), String> {
        let result = self.load_locked();
        match result {
            Ok((data, bytes)) => {
                self.data = data;
                self.snapshot = bytes;
                self.error = None;
                Ok(())
            }
            Err(error) => {
                self.error = Some(error.clone());
                Err(error)
            }
        }
    }

    fn load_locked(&self) -> Result<(Connections, Option<Vec<u8>>), String> {
        let _lock = self.lock()?;
        let path = self.directory.join(FILE_NAME);
        if let Some(bytes) = read_optional(&path)? {
            return Ok((decode(&bytes, &path)?, Some(bytes)));
        }
        // Do not silently resurrect stale legacy data after the new file was lost.
        if self.snapshot.is_some() || self.directory.join("connection-backups").exists() {
            return Err(format!("Saved connections file is missing: {}. Restore a backup before reloading; saving is blocked.", path.display()));
        }
        let legacy = self.directory.join("connections.json");
        if let Some(bytes) = read_optional(&legacy)? {
            let data = decode(&bytes, &legacy)?;
            let migrated = serialize(&data)?;
            self.backup(&bytes)?;
            atomic_write(&path, &migrated)?;
            return Ok((data, Some(migrated)));
        }
        Ok((HashMap::new(), None))
    }

    // OS advisory lock on a separate, never-renamed file. Closing releases it even
    // on process exit; no stale PID file can permanently lock out the user.
    fn lock(&self) -> Result<StoreLock, String> {
        fs::create_dir_all(&self.directory).map_err(|e| io_error(&self.directory, e))?;
        let path = self.directory.join("connections.lock");
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path).map_err(|e| io_error(&path, e))?;
        file.try_lock().map_err(|error| format!("Cannot lock saved connections at {}: {error}. Another instance may be saving. Retry after it finishes; no connections were overwritten.", path.display()))?;
        Ok(StoreLock(file))
    }

    fn backup(&self, bytes: &[u8]) -> Result<(), String> {
        let directory = self.directory.join("connection-backups");
        fs::create_dir_all(&directory).map_err(|e| io_error(&directory, e))?;
        let name = format!(
            "{}-{}.json",
            chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ"),
            Uuid::new_v4()
        );
        // Unique immutable snapshots: repeated saves must not erase the last
        // useful pre-migration/pre-deletion backup. They contain credentials.
        atomic_write(&directory.join(name), bytes)
    }

    fn update(&mut self, change: impl FnOnce(&mut Connections)) -> Result<(), String> {
        self.data()?;
        let _lock = self.lock()?;
        let path = self.directory.join(FILE_NAME);
        let current = read_optional(&path)?;
        if current != self.snapshot {
            return Err(format!("Saved connections changed in another instance or on disk: {}. Reload connections before saving; no connections were overwritten.", path.display()));
        }
        let mut candidate = self.data.clone();
        change(&mut candidate);
        let bytes = serialize(&candidate)?;
        // Back up even an empty first snapshot, which also marks initialization.
        self.backup(current.as_deref().unwrap_or(b"{}"))?;
        atomic_write(&path, &bytes)?;
        // Never publish in-memory edits when backup or persistence failed.
        self.data = candidate;
        self.snapshot = Some(bytes);
        Ok(())
    }

    pub fn upsert(&mut self, mut connection: Connection) -> Result<(), String> {
        self.update(|data| {
            if let Some(stored) = data.get(&connection.id) {
                connection.preserve_secrets_from(stored);
            }
            data.insert(connection.id, connection);
        })
    }

    pub fn remove(&mut self, id: Uuid) -> Result<(), String> {
        self.update(|data| {
            data.remove(&id);
        })
    }

    pub fn import(&mut self, bytes: &[u8]) -> Result<usize, String> {
        let imported = decode(bytes, Path::new("selected import file"))?;
        let count = imported.len();
        self.update(|data| {
            data.extend(imported);
        })?;
        Ok(count)
    }

    pub fn export(&self, path: &Path) -> Result<(), String> {
        let bytes = serialize(self.data()?)?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let parent = fs::canonicalize(parent).map_err(|e| io_error(parent, e))?;
        let protected = fs::canonicalize(&self.protected_root)
            .map_err(|e| io_error(&self.protected_root, e))?;
        let destination = fs::canonicalize(path).ok();
        if parent.starts_with(&protected)
            || destination
                .as_ref()
                .is_some_and(|p| p.starts_with(&protected))
        {
            return Err("Export to a location outside the application's connection storage directory to protect saved connections and backups.".into());
        }
        atomic_write(path, &bytes)
    }
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io_error(path, error)),
    }
}

fn decode(bytes: &[u8], path: &Path) -> Result<Connections, String> {
    // Do not include serde's error text: enum/type errors may echo credentials.
    let data: Connections = serde_json::from_slice(bytes).map_err(|_| format!("Cannot read saved connections from {}: invalid or unsupported format. The file has not been overwritten. Restore a valid backup and reload; saving is blocked.", path.display()))?;
    if data.iter().any(|(id, connection)| id != &connection.id) {
        return Err(format!("Connection IDs do not match in {}. Saving is blocked; the file has not been overwritten.", path.display()));
    }
    Ok(data)
}

fn serialize(data: &Connections) -> Result<Vec<u8>, String> {
    serde_json::to_vec_pretty(data).map_err(|_| "Cannot serialize saved connections".into())
}

fn io_error(path: &Path, error: io::Error) -> String {
    format!("Cannot access connection storage at {}: {error}. Saving is blocked; check permissions and reload.", path.display())
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| io_error(path, e))?;
    // NamedTempFile is private (0600 on Unix) and on the destination filesystem.
    temporary.write_all(bytes).map_err(|e| io_error(path, e))?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|e| io_error(path, e))?;
    temporary
        .persist(path)
        .map_err(|e| io_error(path, e.error))?;
    #[cfg(unix)]
    File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|e| io_error(path, e))?;
    Ok(())
}
