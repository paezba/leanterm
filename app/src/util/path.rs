use std::path::Path;

pub use warp_util::path::*;

/// Returns the file name of `path` for display (e.g. tab titles).
pub fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

/// Returns the display string of `path`.
///
/// When `abbreviate_home` is true, paths under the user's home directory are abbreviated
/// with a `~/` prefix.
pub fn display_path(path: &Path, abbreviate_home: bool) -> String {
    if abbreviate_home {
        dirs::home_dir()
            .and_then(|home| path.strip_prefix(&home).ok())
            .map(|relative| format!("~/{}", relative.display()))
            .unwrap_or_else(|| path.display().to_string())
    } else {
        path.display().to_string()
    }
}
