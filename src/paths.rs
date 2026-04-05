//! Shared filesystem path helpers.

use std::path::PathBuf;

/// Relative path used for cover art storage under the runtime data root.
pub const COVER_ART_RELATIVE_DIR: &str = "covers";

/// Name of the environment variable that stores the runtime data directory.
pub const SUBOXIDE_DATA_DIR_ENV: &str = "SUBOXIDE_DATA_DIR";

/// Build the cover art directory path from a data root directory.
#[must_use]
pub fn cover_art_dir_from_data_dir(data_dir: impl AsRef<std::path::Path>) -> PathBuf {
    data_dir.as_ref().join(COVER_ART_RELATIVE_DIR)
}

/// Resolve the runtime data directory for `suboxide`.
///
/// Priority:
/// 1) `SUBOXIDE_DATA_DIR`
/// 2) `$HOME/.local/share/suboxide`
/// 3) current directory (`.`)
#[must_use]
pub fn resolve_data_dir() -> PathBuf {
    if let Ok(path) = std::env::var(SUBOXIDE_DATA_DIR_ENV)
        && !path.is_empty()
    {
        return PathBuf::from(path);
    }

    if let Some(home) = dirs::home_dir() {
        return home.join(".local/share/suboxide");
    }

    PathBuf::from(".")
}

/// Resolve the cover art directory.
#[must_use]
pub fn resolve_cover_art_dir() -> PathBuf {
    cover_art_dir_from_data_dir(resolve_data_dir())
}
