//! Proves the Phase-2 concurrency pragmas are actually applied.
//!
//! These are regression tests against silently reverting to the pre-WAL
//! defaults, where concurrent readers got SQLITE_BUSY under writer load.

use context_server_rs::db::connection;

#[test]
fn wal_mode_is_enabled_on_file_databases() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("wal_check.db");

    let conn = connection::open(&path).unwrap();
    let mode = connection::journal_mode(&conn).unwrap();
    assert_eq!(
        mode.to_lowercase(),
        "wal",
        "journal_mode should be WAL, got {}",
        mode
    );

    // A real on-disk WAL creates -wal/-shm sidecars on first write. They are
    // checkpointed away on clean close, so check while the connection is live.
    conn.execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY);")
        .unwrap();
    assert!(
        path.with_extension("db-wal").exists(),
        "expected a -wal sidecar file to exist while the WAL connection is open"
    );
}

#[test]
fn busy_timeout_is_configured() {
    let dir = tempfile::tempdir().unwrap();
    let conn = connection::open(dir.path().join("busy_check.db")).unwrap();

    assert_eq!(
        connection::busy_timeout_ms(&conn).unwrap(),
        connection::BUSY_TIMEOUT_MS,
        "busy_timeout should be {}ms",
        connection::BUSY_TIMEOUT_MS
    );
}

#[test]
fn foreign_keys_are_enforced() {
    let dir = tempfile::tempdir().unwrap();
    let conn = connection::open(dir.path().join("fk_check.db")).unwrap();

    assert!(
        connection::foreign_keys_enabled(&conn).unwrap(),
        "foreign_keys must be ON - SQLite disables it by default and all FKs become inert"
    );

    // Behavioural proof, not just the flag.
    conn.execute_batch(
        "CREATE TABLE parent (id TEXT PRIMARY KEY);
         CREATE TABLE child (id TEXT PRIMARY KEY,
                             parent_id TEXT NOT NULL REFERENCES parent(id));",
    )
    .unwrap();

    let orphan = conn.execute("INSERT INTO child (id, parent_id) VALUES ('c1','missing')", []);
    assert!(
        orphan.is_err(),
        "inserting an orphan row should violate the FK constraint"
    );
}

#[test]
fn concurrent_reads_succeed_while_a_writer_holds_the_db() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("concurrency.db");

    // Writer opens a transaction and holds it open.
    let writer = connection::open(&path).unwrap();
    writer
        .execute_batch(
            "CREATE TABLE t (id INTEGER PRIMARY KEY, v TEXT);
             INSERT INTO t (v) VALUES ('a');",
        )
        .unwrap();
    writer
        .execute_batch("BEGIN IMMEDIATE; INSERT INTO t (v) VALUES ('b');")
        .unwrap();

    // A second connection must still be able to READ under WAL.
    let reader = connection::open(&path).unwrap();
    let count: i64 = reader
        .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
        .expect("reader blocked by writer - WAL is not active");
    assert_eq!(count, 1, "reader should see the pre-transaction snapshot");

    writer.execute_batch("COMMIT;").unwrap();
}
