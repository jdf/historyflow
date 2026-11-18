//! Data sources that run outside the browser, and the files they write.
//! Each source turns something with a history into a
//! [`historyflow_core::Bundle`]; [`write`] stores it compactly.

pub mod git_file;

use std::path::Path;

use historyflow_core::Archive;

pub type Error = Box<dyn std::error::Error>;

/// Reads a history from a file of either kind: an archive, or a bundle's
/// JSON.
pub fn read(path: &Path) -> Result<Archive, Error> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Archive::read(&bytes).map_err(|e| format!("{}: {e}", path.display()))?)
}

/// Writes a history to a file: as a bundle's JSON, with every version's
/// text written out, if the name ends in `.json`; otherwise as an
/// archive. Returns the number of bytes written.
pub fn write(archive: &Archive, path: &Path) -> Result<usize, Error> {
    let bytes = if path
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        serde_json::to_vec(&archive.to_bundle())?
    } else {
        archive.to_bytes()
    };
    std::fs::write(path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(bytes.len())
}
