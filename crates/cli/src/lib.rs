//! Data sources that run outside the browser, and the file they write.
//! Each source turns something with a history into versions, which
//! [`write`] stores compactly, as an archive.

pub mod git_file;
pub mod serve;
pub mod wikipedia;

use std::path::Path;

use historyflow_core::Archive;

pub type Error = Box<dyn std::error::Error>;

/// Reads a history from a file: an archive, or the JSON a script may
/// write one as.
pub fn read(path: &Path) -> Result<Archive, Error> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Archive::read(&bytes).map_err(|e| format!("{}: {e}", path.display()))?)
}

/// Whether a file is a history rather than something with one: an
/// archive, or (by its name) a script's JSON.
pub fn is_history_file(path: &Path) -> bool {
    let mut start = [0; 64];
    let read = std::fs::File::open(path)
        .and_then(|mut file| std::io::Read::read(&mut file, &mut start))
        .unwrap_or(0);
    Archive::is_archive(&start[..read]) || path.extension().is_some_and(|e| e == "json")
}

/// Writes a history to a file, as an archive. Returns the number of
/// bytes written.
pub fn write(archive: &Archive, path: &Path) -> Result<usize, Error> {
    let bytes = archive.to_bytes();
    std::fs::write(path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(bytes.len())
}
