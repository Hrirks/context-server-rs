//! The trust boundary around indexing.
//!
//! `index_project` used to accept any path and index it: no canonicalisation, no
//! registration, no containment check. A misbehaving or compromised MCP client
//! could have the server read source from anywhere the user can read — a home
//! directory, another checkout, or a secrets directory with a source-like
//! extension.
//!
//! A root must now be registered against a project before anything under it is
//! indexed, and the canonical path of every index request has to stay inside that
//! root. Registration is explicit rather than inferred, so nothing is trusted
//! merely because it was tried first.

use std::path::{Path, PathBuf};

/// Why an index root was refused.
///
/// Every variant carries enough for the caller to act on. A refusal that does not
/// say what to do instead is what makes people turn a check off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootRejection {
    /// A relative path cannot be checked for containment.
    NotAbsolute(String),
    /// The path does not exist, or could not be resolved through its symlinks.
    Unresolvable(String),
    /// The path exists but is not a directory.
    NotADirectory(String),
    /// The project has no registered root, so nothing under it is trusted yet.
    NotRegistered,
    /// The canonical path lies outside the project's registered root.
    OutsideRegisteredRoot { canonical: String, allowed: String },
    /// Indexing this path would sweep in the server's own stored source.
    ContainsServerStorage { canonical: String, storage: String },
    /// A filesystem root is too broad to be anyone's index scope.
    TooBroad(String),
}

impl std::fmt::Display for RootRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RootRejection::NotAbsolute(path) => write!(
                f,
                "'{path}' is not absolute, so it cannot be checked against a registered root"
            ),
            RootRejection::Unresolvable(path) => {
                write!(f, "'{path}' does not exist or could not be resolved")
            }
            RootRejection::NotADirectory(path) => write!(f, "'{path}' is not a directory"),
            RootRejection::NotRegistered => write!(
                f,
                "this project has no registered root, so no path is trusted for it yet;                  call register_root with the root you intend to index"
            ),
            RootRejection::OutsideRegisteredRoot { canonical, allowed } => write!(
                f,
                "'{canonical}' is outside the registered root '{allowed}';                  register that root explicitly if that is intended"
            ),
            RootRejection::ContainsServerStorage { canonical, storage } => write!(
                f,
                "'{canonical}' contains the server's own storage at '{storage}'"
            ),
            RootRejection::TooBroad(path) => write!(
                f,
                "'{path}' is a filesystem root; register the specific tree you mean instead"
            ),
        }
    }
}

/// Canonicalise a requested index root and confirm it may be indexed.
///
/// Returns the canonical path, which is what callers should record and compare
/// against. Canonicalising first is what makes the symlink case tractable: a path
/// that resolves outside the registered root is refused on its resolved location,
/// not on how it was spelled.
pub fn resolve_index_root(
    requested: &str,
    registered_root: Option<&str>,
    storage_dir: Option<&Path>,
) -> Result<PathBuf, RootRejection> {
    let canonical = canonicalise_dir(requested, storage_dir)?;

    let Some(registered) = registered_root else {
        return Err(RootRejection::NotRegistered);
    };

    let registered = Path::new(registered)
        .canonicalize()
        .map_err(|_| RootRejection::Unresolvable(registered.to_string()))?;

    // Component-wise containment, so a sibling like "app-other" is not treated as
    // being inside "app".
    if canonical == registered || canonical.starts_with(&registered) {
        Ok(canonical)
    } else {
        Err(RootRejection::OutsideRegisteredRoot {
            canonical: canonical.display().to_string(),
            allowed: registered.display().to_string(),
        })
    }
}

/// Where the server keeps its database, derived from the home directory.
///
/// The single source of truth for this path: `main` uses it to place the database
/// and the trust boundary uses it to refuse indexing a tree that would sweep the
/// database in. A second copy of this convention is how the two drift apart.
pub fn default_storage_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join("config").join("context-server-rs"))
}

/// Canonicalise a directory that a project is being asked to register.
///
/// Registration has no root to check containment against yet, so this applies the
/// checks that do not depend on one: the path must be absolute, exist, be a
/// directory, not be a filesystem root, and not bring the server's own storage
/// into scope.
pub fn canonical_registration_root(
    requested: &str,
    storage_dir: Option<&Path>,
) -> Result<PathBuf, RootRejection> {
    canonicalise_dir(requested, storage_dir)
}

fn canonicalise_dir(requested: &str, storage_dir: Option<&Path>) -> Result<PathBuf, RootRejection> {
    let path = Path::new(requested);
    if !path.is_absolute() {
        return Err(RootRejection::NotAbsolute(requested.to_string()));
    }

    let canonical = path
        .canonicalize()
        .map_err(|_| RootRejection::Unresolvable(requested.to_string()))?;

    if !canonical.is_dir() {
        return Err(RootRejection::NotADirectory(
            canonical.display().to_string(),
        ));
    }

    // "/" has no parent. A scan rooted there is never intended, and on any machine
    // it would include the server's own storage.
    if canonical.parent().is_none() {
        return Err(RootRejection::TooBroad(canonical.display().to_string()));
    }

    if let Some(storage) = storage_dir {
        if let Ok(storage) = storage.canonicalize() {
            // Either direction is a problem: indexing the storage directory, or
            // indexing an ancestor of it (a home directory contains it).
            if canonical == storage
                || canonical.starts_with(&storage)
                || storage.starts_with(&canonical)
            {
                return Err(RootRejection::ContainsServerStorage {
                    canonical: canonical.display().to_string(),
                    storage: storage.display().to_string(),
                });
            }
        }
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in for the server's storage directory, which must never be indexed.
    fn storage(dir: &Path) -> PathBuf {
        let storage = dir.join("config").join("context-server-rs");
        std::fs::create_dir_all(&storage).unwrap();
        storage
    }

    #[test]
    fn a_relative_path_is_refused_before_anything_else() {
        let dir = tempfile::tempdir().unwrap();
        let storage = storage(dir.path());

        let error = resolve_index_root("relative/src", Some("/tmp"), Some(&storage)).unwrap_err();
        assert!(matches!(error, RootRejection::NotAbsolute(_)));
    }

    #[test]
    fn an_unregistered_project_trusts_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let storage = storage(dir.path());
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();

        let error = resolve_index_root(repo.to_str().unwrap(), None, Some(&storage)).unwrap_err();
        assert_eq!(error, RootRejection::NotRegistered);
        // The refusal has to name the way forward.
        assert!(error.to_string().contains("register_root"));
    }

    #[test]
    fn a_path_inside_the_registered_root_is_accepted() {
        let dir = tempfile::tempdir().unwrap();
        let storage = storage(dir.path());
        let repo = dir.path().join("repo");
        let nested = repo.join("packages").join("app");
        std::fs::create_dir_all(&nested).unwrap();

        let resolved = resolve_index_root(
            nested.to_str().unwrap(),
            Some(repo.to_str().unwrap()),
            Some(&storage),
        )
        .unwrap();

        assert!(resolved.ends_with("packages/app"));
    }

    #[test]
    fn a_sibling_of_the_registered_root_is_not_inside_it() {
        let dir = tempfile::tempdir().unwrap();
        let storage = storage(dir.path());
        let repo = dir.path().join("app");
        let sibling = dir.path().join("app-other");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();

        let error = resolve_index_root(
            sibling.to_str().unwrap(),
            Some(repo.to_str().unwrap()),
            Some(&storage),
        )
        .unwrap_err();
        assert!(matches!(error, RootRejection::OutsideRegisteredRoot { .. }));
    }

    #[test]
    fn traversal_out_of_the_registered_root_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let storage = storage(dir.path());
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        std::fs::create_dir_all(dir.path().join("elsewhere")).unwrap();

        // Spelled as if it were under the root, resolves to a sibling of it. Note
        // that "repo/nested/.." would land back *inside* "repo" and prove nothing;
        // the traversal has to actually leave.
        let escape = format!("{}/../elsewhere", repo.display());
        let error =
            resolve_index_root(&escape, Some(repo.to_str().unwrap()), Some(&storage)).unwrap_err();
        assert!(matches!(error, RootRejection::OutsideRegisteredRoot { .. }));
    }

    #[test]
    fn the_servers_own_storage_cannot_be_indexed() {
        let dir = tempfile::tempdir().unwrap();
        let storage = storage(dir.path());

        // The storage directory itself.
        let error = resolve_index_root(
            storage.to_str().unwrap(),
            Some(dir.path().to_str().unwrap()),
            Some(&storage),
        )
        .unwrap_err();
        assert!(matches!(error, RootRejection::ContainsServerStorage { .. }));

        // An ancestor of it, which would sweep it in.
        let error = resolve_index_root(
            dir.path().to_str().unwrap(),
            Some(dir.path().to_str().unwrap()),
            Some(&storage),
        )
        .unwrap_err();
        assert!(matches!(error, RootRejection::ContainsServerStorage { .. }));
    }

    #[test]
    fn a_filesystem_root_is_refused_as_too_broad() {
        let error = resolve_index_root("/", None, None).unwrap_err();
        assert!(matches!(error, RootRejection::TooBroad(_)));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_that_resolves_outside_the_root_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let storage = storage(dir.path());
        let repo = dir.path().join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let outside = dir.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();

        let link = repo.join("escape");
        std::os::unix::fs::symlink(&outside, &link).unwrap();

        // Spelled inside the registered root, resolved outside it.
        let error = resolve_index_root(
            link.to_str().unwrap(),
            Some(repo.to_str().unwrap()),
            Some(&storage),
        )
        .unwrap_err();
        assert!(matches!(error, RootRejection::OutsideRegisteredRoot { .. }));
    }
}
