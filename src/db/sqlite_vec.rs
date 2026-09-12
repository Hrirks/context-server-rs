//! Process-global registration of the [`sqlite_vec`] extension.
//!
//! sqlite-vec supplies the `vec_distance_cosine()` (and friends) SQL functions
//! that let SQLite rank stored embeddings without loading every vector into
//! Rust. Registering it as an auto-extension means every connection opened
//! afterwards exposes those functions, so callers never have to remember to
//! load it.

use rusqlite::ffi::{sqlite3, sqlite3_api_routines};
use std::os::raw::{c_char, c_int};
use std::sync::Once;

/// The signature `sqlite3_auto_extension` expects for an extension entry point.
type ExtensionInit =
    unsafe extern "C" fn(*mut sqlite3, *mut *const c_char, *const sqlite3_api_routines) -> c_int;

static REGISTER: Once = Once::new();

/// Register sqlite-vec as a process-global SQLite auto-extension. Idempotent.
///
/// `sqlite3_auto_extension` only affects connections opened *after* the call,
/// so this must run before the first [`rusqlite::Connection::open`]. It is
/// invoked from [`crate::db::connection::open`] for exactly that reason.
pub fn register() {
    REGISTER.call_once(|| unsafe {
        // `sqlite3_vec_init` is exported with the standard extension-entry
        // signature; the crate does not name that type, so we cast through a
        // fn pointer we declare ourselves.
        let init: ExtensionInit = std::mem::transmute::<*const (), ExtensionInit>(
            ::sqlite_vec::sqlite3_vec_init as *const (),
        );
        rusqlite::ffi::sqlite3_auto_extension(Some(init));
    });
}
