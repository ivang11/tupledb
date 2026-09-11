use app_lib::connection_store::{config_directory, ConnectionStore, Connections};
use app_lib::connections::Connection;
use serde_json::json;
use std::fs;
use tempfile::TempDir;
use uuid::Uuid;

fn connection() -> Connection {
    serde_json::from_value(json!({
        "id": Uuid::new_v4(), "name": "Synthetic", "environment": "LOCAL",
        "mysql": {"host": "localhost", "port": 3306, "user": "test", "password": "synthetic-password"},
        "ssh": {"host": "localhost", "port": 22, "user": "test", "auth": {"type": "password", "password": "synthetic-ssh"}}
    })).unwrap()
}

fn legacy(count: usize) -> Vec<u8> {
    let mut map = serde_json::Map::new();
    for _ in 0..count {
        let c = connection();
        let mut value = serde_json::to_value(&c).unwrap();
        value["mysql"] = value["database"]["settings"].clone();
        value.as_object_mut().unwrap().remove("database");
        map.insert(c.id.to_string(), value);
    }
    serde_json::to_vec_pretty(&map).unwrap()
}

fn open(dir: &TempDir) -> ConnectionStore {
    ConnectionStore::open(dir.path().into())
}
fn primary(dir: &TempDir) -> Vec<u8> {
    fs::read(dir.path().join("connections.v2.json")).unwrap()
}
fn backups(dir: &TempDir) -> Vec<Vec<u8>> {
    fs::read_dir(dir.path().join("connection-backups"))
        .unwrap()
        .map(|p| fs::read(p.unwrap().path()).unwrap())
        .collect()
}

#[test]
fn migrates_twelve_legacy_connections_without_rewriting_the_original() {
    let dir = TempDir::new().unwrap();
    let original = legacy(12);
    fs::write(dir.path().join("connections.json"), &original).unwrap();
    let mut store = open(&dir);
    assert_eq!(store.data().unwrap().len(), 12);
    assert_eq!(backups(&dir), vec![original.clone()]);
    store.upsert(connection()).unwrap();
    assert_eq!(open(&dir).data().unwrap().len(), 13);
    assert_eq!(
        fs::read(dir.path().join("connections.json")).unwrap(),
        original
    );
    let records: serde_json::Value = serde_json::from_slice(&original).unwrap();
    assert!(records
        .as_object()
        .unwrap()
        .values()
        .all(|v| v.get("mysql").is_some()));
}

#[test]
fn an_old_version_overwriting_legacy_cannot_erase_the_migrated_connections() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("connections.json"), legacy(12)).unwrap();
    let mut store = open(&dir);
    store.upsert(connection()).unwrap();
    fs::write(dir.path().join("connections.json"), legacy(2)).unwrap();
    assert_eq!(open(&dir).data().unwrap().len(), 13);
}

#[test]
fn development_does_not_read_or_write_installed_app_connections() {
    let dir = TempDir::new().unwrap();
    let original = legacy(12);
    fs::write(dir.path().join("connections.json"), &original).unwrap();
    let release = config_directory(dir.path().into(), false);
    let development = config_directory(dir.path().into(), true);
    assert_eq!(release, dir.path());
    let mut dev = ConnectionStore::open(development.clone());
    assert!(dev.data().unwrap().is_empty());
    dev.upsert(connection()).unwrap();
    assert_eq!(ConnectionStore::open(development).data().unwrap().len(), 1);
    assert!(!dir.path().join("connections.v2.json").exists());
    assert_eq!(
        fs::read(dir.path().join("connections.json")).unwrap(),
        original
    );
}

#[test]
fn malformed_or_unsupported_files_block_all_writes_and_exports_without_echoing_secrets() {
    for bytes in [
        b"not-json synthetic-password".as_slice(),
        b"[]",
        b"{",
        b"null",
    ] {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("connections.v2.json"), bytes).unwrap();
        let mut store = open(&dir);
        let error = store.data().unwrap_err();
        assert!(error.contains("Saving is blocked") || error.contains("saving is blocked"));
        assert!(!error.contains("synthetic-password"));
        assert!(store.upsert(connection()).is_err());
        assert!(store.remove(Uuid::new_v4()).is_err());
        assert!(store.import(&legacy(1)).is_err());
        let export_dir = TempDir::new().unwrap();
        let target = export_dir.path().join("export.json");
        fs::write(&target, b"existing export").unwrap();
        assert!(store.export(&target).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"existing export");
        assert_eq!(primary(&dir), bytes);
        assert!(!dir.path().join("connection-backups").exists());
    }
}

#[test]
fn one_invalid_record_does_not_silently_discard_the_other_eleven() {
    let dir = TempDir::new().unwrap();
    let mut data: serde_json::Value = serde_json::from_slice(&legacy(12)).unwrap();
    data.as_object_mut().unwrap().values_mut().next().unwrap()["environment"] =
        json!("synthetic-secret-invalid-enum");
    let bytes = serde_json::to_vec(&data).unwrap();
    fs::write(dir.path().join("connections.json"), &bytes).unwrap();
    let mut store = open(&dir);
    assert!(!store.data().unwrap_err().contains("synthetic-secret"));
    assert!(store.upsert(connection()).is_err());
    assert!(!dir.path().join("connections.v2.json").exists());
    assert_eq!(
        fs::read(dir.path().join("connections.json")).unwrap(),
        bytes
    );
}

#[test]
fn does_not_fall_back_to_legacy_if_the_versioned_file_is_unreadable() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("connections.json"), legacy(12)).unwrap();
    // A directory at the file path reliably simulates an IO read failure, even as root.
    fs::create_dir(dir.path().join("connections.v2.json")).unwrap();
    let mut store = open(&dir);
    assert!(store.data().is_err());
    assert!(store.upsert(connection()).is_err());
    assert!(dir.path().join("connections.v2.json").is_dir());
}

#[test]
fn a_failed_load_stays_blocked_until_an_explicit_successful_reload() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("connections.v2.json"), b"broken").unwrap();
    let mut store = open(&dir);
    let restored = legacy(12);
    fs::write(dir.path().join("connections.v2.json"), &restored).unwrap();
    assert!(store.upsert(connection()).is_err());
    assert_eq!(primary(&dir), restored);
    store.reload().unwrap();
    store.upsert(connection()).unwrap();
    assert_eq!(store.data().unwrap().len(), 13);
}

#[test]
fn stale_instances_cannot_overwrite_additions_deletions_or_imports() {
    let dir = TempDir::new().unwrap();
    let mut first = open(&dir);
    let mut stale = open(&dir);
    let saved = connection();
    first.upsert(saved.clone()).unwrap();
    let before = primary(&dir);
    assert!(stale
        .upsert(connection())
        .unwrap_err()
        .contains("Reload connections"));
    assert!(stale.remove(saved.id).is_err());
    assert!(stale.import(&legacy(2)).is_err());
    assert_eq!(primary(&dir), before);
    assert!(stale.data().unwrap().is_empty());
    stale.reload().unwrap();
    stale.upsert(connection()).unwrap();
    assert_eq!(stale.data().unwrap().len(), 2);
    assert!(first.remove(saved.id).is_err());
    first.reload().unwrap();
    first.remove(saved.id).unwrap();
    assert_eq!(first.data().unwrap().len(), 1);
    assert!(stale.upsert(connection()).is_err());
    assert_eq!(open(&dir).data().unwrap().len(), 1);
}

#[test]
fn missing_file_after_initialization_never_resets_to_an_empty_or_legacy_list() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("connections.json"), legacy(12)).unwrap();
    let mut store = open(&dir);
    fs::remove_file(dir.path().join("connections.v2.json")).unwrap();
    assert!(store.upsert(connection()).is_err());
    assert!(store.reload().is_err());
    assert!(open(&dir).data().is_err());
    assert!(!dir.path().join("connections.v2.json").exists());
    assert_eq!(backups(&dir).len(), 1);
}

#[test]
fn backup_failure_does_not_publish_edits_in_memory_or_on_disk() {
    let dir = TempDir::new().unwrap();
    let mut store = open(&dir);
    store.import(&legacy(12)).unwrap();
    let before = primary(&dir);
    fs::rename(
        dir.path().join("connection-backups"),
        dir.path().join("retained-backups"),
    )
    .unwrap();
    fs::write(
        dir.path().join("connection-backups"),
        b"blocked backup directory",
    )
    .unwrap();
    assert!(store.upsert(connection()).is_err());
    assert_eq!(store.data().unwrap().len(), 12);
    assert_eq!(primary(&dir), before);
}

#[test]
fn development_exports_cannot_target_the_installed_apps_storage() {
    let dir = TempDir::new().unwrap();
    let original = legacy(12);
    fs::write(dir.path().join("connections.json"), &original).unwrap();
    let mut dev = ConnectionStore::open_profile(dir.path().into(), true);
    dev.upsert(connection()).unwrap();
    assert!(dev.export(&dir.path().join("connections.json")).is_err());
    assert!(dev.export(&dir.path().join("connections.v2.json")).is_err());
    assert_eq!(
        fs::read(dir.path().join("connections.json")).unwrap(),
        original
    );
}

#[test]
fn migrates_tagged_mysql_and_postgresql_records_from_earlier_development_builds() {
    let dir = TempDir::new().unwrap();
    let mysql = connection();
    let pg: Connection = serde_json::from_value(json!({
        "id": Uuid::new_v4(), "name": "PostgreSQL fixture", "environment": "LOCAL",
        "database": {"engine": "postgresql", "settings": {"host": "localhost", "port": 5432, "user": "test", "ssl_mode": "require"}}
    })).unwrap();
    let expected: Connections = [(mysql.id, mysql), (pg.id, pg)].into_iter().collect();
    let bytes = serde_json::to_vec(&expected).unwrap();
    fs::write(dir.path().join("connections.json"), &bytes).unwrap();
    let store = open(&dir);
    assert_eq!(
        serde_json::to_value(store.data().unwrap()).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    assert_eq!(
        fs::read(dir.path().join("connections.json")).unwrap(),
        bytes
    );
}

#[test]
fn simultaneous_writers_cannot_both_commit_their_original_empty_snapshot() {
    use std::sync::{Arc, Barrier};
    let dir = TempDir::new().unwrap();
    let first = open(&dir);
    let second = open(&dir);
    let barrier = Arc::new(Barrier::new(2));
    let handles: Vec<_> = [first, second]
        .into_iter()
        .map(|mut store| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                store.upsert(connection()).is_ok()
            })
        })
        .collect();
    let successes = handles
        .into_iter()
        .map(|h| usize::from(h.join().unwrap()))
        .sum::<usize>();
    assert_eq!(successes, 1);
    assert_eq!(open(&dir).data().unwrap().len(), 1);
    assert_eq!(backups(&dir).len(), 1);
}

#[test]
fn backups_keep_every_previous_snapshot_including_deleted_connections_and_secrets() {
    let dir = TempDir::new().unwrap();
    let mut store = open(&dir);
    let c = connection();
    store.upsert(c.clone()).unwrap();
    let before_delete = primary(&dir);
    store.remove(c.id).unwrap();
    store.upsert(connection()).unwrap();
    let copies = backups(&dir);
    assert_eq!(copies.len(), 3);
    assert!(copies.contains(&before_delete));
    let restored: Connections = serde_json::from_slice(&before_delete).unwrap();
    assert_eq!(
        serde_json::to_value(&restored[&c.id]).unwrap(),
        serde_json::to_value(&c).unwrap()
    );
}

#[test]
fn invalid_imports_and_mismatched_ids_leave_the_entire_store_unchanged() {
    let dir = TempDir::new().unwrap();
    let mut store = open(&dir);
    store.import(&legacy(12)).unwrap();
    let before = primary(&dir);
    let wrong_id = json!({Uuid::new_v4().to_string(): connection()});
    assert!(store
        .import(&serde_json::to_vec(&wrong_id).unwrap())
        .is_err());
    assert!(store.import(b"invalid").is_err());
    assert_eq!(primary(&dir), before);
    assert_eq!(store.data().unwrap().len(), 12);
}

#[test]
fn edits_preserve_hidden_passwords_and_export_can_be_imported() {
    let dir = TempDir::new().unwrap();
    let mut store = open(&dir);
    let c = connection();
    store.upsert(c.clone()).unwrap();
    let mut edited = c.clone();
    edited.name = "Edited".into();
    edited.database.strip_password();
    if let Some(ssh) = &mut edited.ssh {
        ssh.auth = app_lib::connections::SshAuth::Password {
            password: String::new(),
        };
    }
    store.upsert(edited).unwrap();
    let value = serde_json::to_value(&store.data().unwrap()[&c.id]).unwrap();
    assert_eq!(
        value["database"]["settings"]["password"],
        "synthetic-password"
    );
    assert_eq!(value["ssh"]["auth"]["password"], "synthetic-ssh");
    let export_dir = TempDir::new().unwrap();
    let path = export_dir.path().join("export.json");
    store.export(&path).unwrap();
    let restored_dir = TempDir::new().unwrap();
    let mut restored = open(&restored_dir);
    assert_eq!(restored.import(&fs::read(path).unwrap()).unwrap(), 1);
    assert_eq!(
        serde_json::to_value(&restored.data().unwrap()[&c.id]).unwrap(),
        value
    );
}

#[test]
fn exports_cannot_overwrite_managed_connections_locks_or_backups() {
    let dir = TempDir::new().unwrap();
    let mut store = open(&dir);
    store.upsert(connection()).unwrap();
    let before = primary(&dir);
    for name in [
        "connections.v2.json",
        "connections.json",
        "connections.lock",
        "connection-backups/export.json",
    ] {
        assert!(store.export(&dir.path().join(name)).is_err());
    }
    assert_eq!(primary(&dir), before);
}

#[cfg(unix)]
#[test]
fn newly_written_connections_and_backups_are_private() {
    use std::os::unix::fs::PermissionsExt;
    let dir = TempDir::new().unwrap();
    let mut store = open(&dir);
    store.upsert(connection()).unwrap();
    let paths = std::iter::once(dir.path().join("connections.v2.json")).chain(
        fs::read_dir(dir.path().join("connection-backups"))
            .unwrap()
            .map(|e| e.unwrap().path()),
    );
    for path in paths {
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}

#[test]
fn an_os_lock_in_another_process_blocks_writes_and_is_released_on_exit() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    let dir = TempDir::new().unwrap();
    let mut store = open(&dir);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "storage_lock_child", "--ignored", "--nocapture"])
        .env("TUPLEDB_STORAGE_LOCK_FIXTURE", dir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        assert!(
            output.read_line(&mut line).unwrap() > 0,
            "child exited before acquiring lock"
        );
        if line.contains("LOCK_READY") {
            break;
        }
        line.clear();
    }
    let rejected = store.upsert(connection()).is_err();
    child.stdin.take().unwrap().write_all(b"exit\n").unwrap();
    assert!(child.wait().unwrap().success());
    assert!(rejected);
    assert!(store.data().unwrap().is_empty());
    store.upsert(connection()).unwrap();
    assert_eq!(open(&dir).data().unwrap().len(), 1);
}

#[test]
#[ignore = "subprocess helper invoked by the lock regression test"]
fn storage_lock_child() {
    use std::io::Write;
    let directory = std::env::var_os("TUPLEDB_STORAGE_LOCK_FIXTURE").unwrap();
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(std::path::PathBuf::from(directory).join("connections.lock"))
        .unwrap();
    file.try_lock().unwrap();
    println!("LOCK_READY");
    std::io::stdout().flush().unwrap();
    let mut line = String::new();
    std::io::stdin().read_line(&mut line).unwrap();
    // Dropping the descriptor (including process exit) releases the OS lock.
}
