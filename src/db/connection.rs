//! Centralized SQLite connection configuration.
//!
//! Every connection the server opens MUST go through [open] (or be passed to
//! [configure]) so the pragma set below is applied consistently. Opening a bare
//! Connection::open anywhere else silently opts that connection out of WAL, the
//! busy timeout, and foreign-key enforcement.

use rusqlite::{Connection, Result};
use std::path::Path;

/// Milliseconds SQLite will wait for a held lock before returning SQLITE_BUSY.
///
/// Without this, concurrent agent queries against a mid-write database fail
/// instantly instead of waiting for the writer to finish.
pub const BUSY_TIMEOUT_MS: u64 = 5_000;

/// Open a SQLite connection with the production pragma set already applied.
pub fn open<P: AsRef<Path>>(path: P) -> Result<Connection> {
    let conn = Connection::open(path)?;
    configure(&conn)?;
    Ok(conn)
}

/// Apply the production pragma set to an existing connection. Idempotent.
///
///   journal_mode=WAL    readers never block the writer, writer never blocks readers
///   busy_timeout=5000   wait for a lock instead of failing fast with SQLITE_BUSY
///   synchronous=NORMAL  durable with WAL, far cheaper than FULL
///   foreign_keys=ON     SQLite defaults this OFF - every FK in this schema was inert
///   temp_store=MEMORY   keep temp b-trees for sorts/joins off disk
pub fn configure(conn: &Connection) -> Result<()> {
    // PRAGMA journal_mode RETURNS a row ("wal"), so it cannot go through
    // execute_batch - it must be stepped as a query.
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;

    conn.execute_batch(&format!(
        "PRAGMA busy_timeout = {BUSY_TIMEOUT_MS};
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA temp_store = MEMORY;"
    ))?;
    Ok(())
}

/// Read back the active journal mode (used by tests to prove WAL is really on).
#[allow(dead_code)]
pub fn journal_mode(conn: &Connection) -> Result<String> {
    conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))
}

/// Read back the active busy timeout in milliseconds.
#[allow(dead_code)]
pub fn busy_timeout_ms(conn: &Connection) -> Result<u64> {
    conn.query_row("PRAGMA busy_timeout", [], |row| {
        Ok(row.get::<_, i64>(0)? as u64)
    })
}

/// Read back whether foreign-key enforcement is active.
#[allow(dead_code)]
pub fn foreign_keys_enabled(conn: &Connection) -> Result<bool> {
    conn.query_row("PRAGMA foreign_keys", [], |row| {
        Ok(row.get::<_, i64>(0)? != 0)
    })
}
