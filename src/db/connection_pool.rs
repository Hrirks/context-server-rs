//! A bounded, blocking SQLite connection pool.
//!
//! This is a correct rewrite of the abandoned `origin/enhancements` pool. The
//! original was unsound: `get_connection` removed a connection from the pool
//! vector, so the max-connections bound counted only *idle* connections and
//! could never cap the pool; it also busy-spun with `thread::sleep`. This
//! version keeps every connection under a single mutex and uses a `Condvar` to
//! block callers until one is released.
//!
//! Each checkout yields an [`Arc<Mutex<Connection>>`] handle (matching the
//! `Arc<Mutex<Connection>>` shape used everywhere else in this codebase), so
//! existing `db.lock().unwrap()` call sites work unchanged.

use crate::db::connection;
use rusqlite::Connection;
use std::ops::Deref;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};
use thiserror::Error;

/// Errors raised while acquiring a connection.
#[derive(Debug, Error)]
pub enum PoolError {
    #[error("timed out after {0:?} waiting for a free connection")]
    TimedOut(Duration),
    #[error("connection error: {0}")]
    Connection(#[from] rusqlite::Error),
}

/// A point-in-time snapshot of pool usage.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub struct PoolStats {
    pub total: usize,
    pub idle: usize,
    pub in_use: usize,
    pub max: usize,
}

struct PoolInner {
    db_path: String,
    max: usize,
    acquire_timeout: Duration,
    state: Mutex<PoolState>,
    available: Condvar,
}

struct PoolState {
    idle: Vec<Arc<Mutex<Connection>>>,
    total: usize,
}

/// A bounded pool of SQLite connections.
pub struct ConnectionPool {
    inner: Arc<PoolInner>,
}

impl ConnectionPool {
    /// Create a pool holding at most `max` connections.
    ///
    /// One connection is opened eagerly so an invalid `db_path` fails here
    /// rather than on first checkout.
    pub fn new(
        db_path: impl Into<String>,
        max: usize,
        acquire_timeout: Duration,
    ) -> Result<Self, PoolError> {
        let max = max.max(1);
        let db_path = db_path.into();
        let handle = Arc::new(Mutex::new(connection::open(&db_path)?));
        let inner = Arc::new(PoolInner {
            db_path,
            max,
            acquire_timeout,
            state: Mutex::new(PoolState {
                idle: vec![handle],
                total: 1,
            }),
            available: Condvar::new(),
        });
        Ok(Self { inner })
    }

    /// Check out a connection, blocking up to the configured acquire timeout.
    pub fn checkout(&self) -> Result<PooledConnection, PoolError> {
        let deadline = Instant::now() + self.inner.acquire_timeout;
        let mut state = self.inner.state.lock().unwrap();

        loop {
            if let Some(handle) = state.idle.pop() {
                return Ok(PooledConnection {
                    handle: Some(handle),
                    inner: Arc::clone(&self.inner),
                });
            }

            if state.total < self.inner.max {
                let handle = Arc::new(Mutex::new(connection::open(&self.inner.db_path)?));
                state.total += 1;
                return Ok(PooledConnection {
                    handle: Some(handle),
                    inner: Arc::clone(&self.inner),
                });
            }

            let now = Instant::now();
            if now >= deadline {
                return Err(PoolError::TimedOut(self.inner.acquire_timeout));
            }

            let (guard, _) = self
                .inner
                .available
                .wait_timeout(state, deadline - now)
                .unwrap();
            state = guard;
        }
    }

    /// Snapshot of current pool usage.
    #[allow(dead_code)]
    pub fn stats(&self) -> PoolStats {
        let state = self.inner.state.lock().unwrap();
        PoolStats {
            total: state.total,
            idle: state.idle.len(),
            in_use: state.total - state.idle.len(),
            max: self.inner.max,
        }
    }
}

/// A connection checked out of the pool. Returning to the pool happens on drop.
pub struct PooledConnection {
    handle: Option<Arc<Mutex<Connection>>>,
    inner: Arc<PoolInner>,
}

impl Deref for PooledConnection {
    type Target = Arc<Mutex<Connection>>;

    fn deref(&self) -> &Self::Target {
        self.handle
            .as_ref()
            .expect("pooled connection already released")
    }
}

impl Drop for PooledConnection {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.inner.state.lock().unwrap().idle.push(handle);
            self.inner.available.notify_one();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn pool(max: usize) -> ConnectionPool {
        ConnectionPool::new(":memory:", max, Duration::from_millis(200)).unwrap()
    }

    #[test]
    fn checkout_and_release() {
        let p = pool(2);
        let c = p.checkout().unwrap();
        {
            // The handle is usable through deref to Arc<Mutex<Connection>>.
            let _guard = c.lock().unwrap();
        }
        assert_eq!(p.stats().in_use, 1);
        drop(c);
        assert_eq!(p.stats().in_use, 0);
        assert_eq!(p.stats().idle, 1);
    }

    #[test]
    fn pool_is_bounded_and_times_out() {
        let p = ConnectionPool::new(":memory:", 1, Duration::from_millis(50)).unwrap();
        let held = p.checkout().unwrap();

        let start = Instant::now();
        assert!(matches!(p.checkout(), Err(PoolError::TimedOut(_))));
        assert!(start.elapsed() >= Duration::from_millis(10));

        drop(held);
        assert!(p.checkout().is_ok());
    }

    #[test]
    fn connections_share_the_database() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let p = ConnectionPool::new(path.to_str().unwrap(), 2, Duration::from_secs(1)).unwrap();

        let c1 = p.checkout().unwrap();
        c1.lock()
            .unwrap()
            .execute_batch("CREATE TABLE t (id INTEGER PRIMARY KEY); INSERT INTO t VALUES (42);")
            .unwrap();
        drop(c1);

        let c2 = p.checkout().unwrap();
        let n: i64 = c2
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM t", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn stats_reflect_pool_state() {
        let p = pool(2);
        assert_eq!(p.stats().total, 1);
        assert_eq!(p.stats().idle, 1);
        assert_eq!(p.stats().in_use, 0);

        let c1 = p.checkout().unwrap();
        assert_eq!(p.stats().in_use, 1);

        let c2 = p.checkout().unwrap();
        assert_eq!(p.stats().total, 2);
        assert_eq!(p.stats().in_use, 2);

        drop(c1);
        drop(c2);
        assert_eq!(p.stats().idle, 2);
        assert_eq!(p.stats().in_use, 0);
    }
}
