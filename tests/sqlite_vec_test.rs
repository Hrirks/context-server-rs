//! Integration tests for the sqlite-vec extension wiring.
//!
//! These prove the extension is registered for connections opened through the
//! server's own opener and pooled connections, not just bare `rusqlite` ones.

use context_server_rs::db::connection_pool::ConnectionPool;
use std::time::Duration;

#[test]
fn pooled_connections_expose_sqlite_vec() {
    let pool = ConnectionPool::new(":memory:", 1, Duration::from_millis(500)).unwrap();
    let conn = pool.checkout().unwrap();
    let db = conn.lock().unwrap();

    let version: String = db
        .query_row("SELECT vec_version()", [], |row| row.get(0))
        .expect("vec_version() should be available on pooled connections");
    assert!(
        version.starts_with('v'),
        "unexpected sqlite-vec version: {version}"
    );

    // Identical vectors have zero cosine distance; orthogonal vectors have 1.
    let identical: f64 = db
        .query_row(
            "SELECT vec_distance_cosine('[1,2,3]', '[1,2,3]')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(identical.abs() < 1e-6);

    let orthogonal: f64 = db
        .query_row(
            "SELECT vec_distance_cosine('[1,0,0]', '[0,1,0]')",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!((orthogonal - 1.0).abs() < 1e-6);
}
