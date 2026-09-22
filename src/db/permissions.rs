//! Permissions for the at-rest store.
//!
//! The database holds symbol source and embeddings of it, and it was created with
//! `create_dir_all` under whatever umask the user happened to have. On a shared
//! machine that is a readable copy of the code. Nothing here is subtle: the
//! directory is owner-only, and so is the database and everything SQLite keeps
//! beside it.

use std::path::{Path, PathBuf};

/// Directory mode: owner read/write/execute, nothing for anyone else.
const DIR_MODE: u32 = 0o700;
/// File mode: owner read/write, nothing for anyone else.
const FILE_MODE: u32 = 0o600;

#[cfg(unix)]
fn restrict(path: &Path, mode: u32) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode))
}

#[cfg(not(unix))]
fn restrict(_path: &Path, _mode: u32) -> std::io::Result<()> {
    // Windows has no equivalent mode here; ACLs are out of scope for this change.
    Ok(())
}

/// Paths SQLite keeps beside the database: the write-ahead log and its index.
///
/// WAL is enabled for the connection, so these exist in normal operation and hold
/// the same source text as the database. Restricting only `context.db` would leave
/// the content readable next to it.
fn sidecar_paths(db_path: &Path) -> Vec<PathBuf> {
    let name = match db_path.file_name().and_then(|n| n.to_str()) {
        Some(name) => name,
        None => return Vec::new(),
    };
    vec![
        db_path.to_path_buf(),
        db_path.with_file_name(format!("{name}-wal")),
        db_path.with_file_name(format!("{name}-shm")),
    ]
}

/// Restrict the storage directory and the database files to their owner.
///
/// Returns the paths that were restricted, so a caller can report what it did
/// rather than asserting that it did something. Failing is deliberate: an
/// unprotected store should stop the server rather than quietly continue.
pub fn harden_storage(dir: &Path, db_path: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut restricted = Vec::new();

    if dir.exists() {
        restrict(dir, DIR_MODE)?;
        restricted.push(dir.to_path_buf());
    }

    for path in sidecar_paths(db_path) {
        // Sidecars appear once a connection has written, so a missing one is
        // normal on a fresh start.
        if path.exists() {
            restrict(&path, FILE_MODE)?;
            restricted.push(path);
        }
    }

    Ok(restricted)
}

/// Environment variable that starts the server with an unprotected store anyway.
pub const ENV_ALLOW_INSECURE_PERMISSIONS: &str = "CONTEXT_ALLOW_INSECURE_PERMISSIONS";

/// Restrict the storage that `db_path` lives in, or refuse to continue.
///
/// Owns the policy as well as the mechanics so there is one place to look: the
/// store is owner-only unless `CONTEXT_ALLOW_INSECURE_PERMISSIONS` says
/// otherwise, and the refusal explains what was wrong and what setting the
/// variable would cost. An in-memory database has no files to protect.
pub fn enforce_or_refuse(db_path: &Path) -> Result<Vec<PathBuf>, String> {
    let Some(dir) = db_path.parent() else {
        return Ok(Vec::new());
    };
    if dir.as_os_str().is_empty() {
        // ":memory:" and friends: nothing on disk to restrict.
        return Ok(Vec::new());
    }

    if std::env::var(ENV_ALLOW_INSECURE_PERMISSIONS).is_ok() {
        tracing::warn!(
            "{ENV_ALLOW_INSECURE_PERMISSIONS} is set: stored source is left readable by \
             other users on this machine"
        );
        return Ok(Vec::new());
    }

    harden_storage(dir, db_path).map_err(|e| {
        format!(
            "could not restrict storage permissions at {}: {e}. Set \
             {ENV_ALLOW_INSECURE_PERMISSIONS}=1 to start anyway, accepting that the stored \
             source is readable by other users.",
            dir.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn mode_of(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    #[cfg(unix)]
    #[test]
    fn the_directory_and_database_become_owner_only() {
        let dir = tempfile::tempdir().unwrap();
        let storage = dir.path().join("context-server-rs");
        std::fs::create_dir_all(&storage).unwrap();
        let db = storage.join("context.db");
        std::fs::write(&db, b"sqlite").unwrap();
        // Sidecars, as WAL mode leaves behind.
        std::fs::write(storage.join("context.db-wal"), b"wal").unwrap();
        std::fs::write(storage.join("context.db-shm"), b"shm").unwrap();

        let restricted = harden_storage(&storage, &db).unwrap();

        assert_eq!(mode_of(&storage), 0o700);
        assert_eq!(mode_of(&db), 0o600);
        assert_eq!(mode_of(&storage.join("context.db-wal")), 0o600);
        assert_eq!(mode_of(&storage.join("context.db-shm")), 0o600);
        // The sidecars are easy to forget, so check they were actually reported.
        assert!(restricted.iter().any(|p| p.ends_with("context.db-wal")));
    }

    #[cfg(unix)]
    #[test]
    fn a_world_readable_database_is_tightened() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("context.db");
        std::fs::write(&db, b"sqlite").unwrap();
        std::fs::set_permissions(&db, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(mode_of(&db), 0o644);

        harden_storage(dir.path(), &db).unwrap();

        assert_eq!(mode_of(&db), 0o600);
    }

    #[test]
    fn an_in_memory_database_has_nothing_to_protect() {
        let restricted = enforce_or_refuse(Path::new(":memory:")).unwrap();
        assert!(restricted.is_empty());
    }

    #[test]
    fn a_missing_sidecar_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("context.db");
        std::fs::write(&db, b"sqlite").unwrap();

        // No -wal/-shm yet: a fresh database has not written through WAL.
        let restricted = harden_storage(dir.path(), &db).unwrap();
        assert!(restricted.iter().any(|p| p.ends_with("context.db")));
        assert!(!restricted.iter().any(|p| p.ends_with("-wal")));
    }
}
