//! Embeds the viewer — the page and its WebAssembly, as tools/build.sh
//! leaves them in build/release/dist — in the command, so that it can
//! serve them with nothing else installed and no network.
//!
//! Writes `viewer.rs`: a list of each file's path and bytes. If the
//! viewer has not been built the list is empty and the command says so
//! when asked to serve it; `tools/build-cli.sh` builds both in order.
//! `HISTORYFLOW_DIST` names another directory to embed.

use std::path::{Path, PathBuf};

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let dist = match std::env::var("HISTORYFLOW_DIST") {
        Ok(dir) => PathBuf::from(dir),
        Err(_) => manifest.join("../../build/release/dist"),
    };
    println!("cargo:rerun-if-env-changed=HISTORYFLOW_DIST");
    println!("cargo:rerun-if-changed={}", dist.display());

    let mut files = Vec::new();
    collect(&dist, &dist, &mut files);
    files.sort();
    let mut source = String::from("pub static VIEWER: &[(&str, &[u8])] = &[\n");
    for (name, path) in files {
        println!("cargo:rerun-if-changed={}", path.display());
        source.push_str(&format!("    ({name:?}, include_bytes!({path:?})),\n"));
    }
    source.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    std::fs::write(out.join("viewer.rs"), source).unwrap();
}

/// Every file under `dir`, as its path relative to `root` with forward
/// slashes, and its absolute path.
fn collect(root: &Path, dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            println!("cargo:rerun-if-changed={}", path.display());
            collect(root, &path, files);
        } else if entry.file_name() != ".DS_Store" {
            let relative = path.strip_prefix(root).unwrap();
            let name: Vec<_> = relative.iter().map(|part| part.to_string_lossy()).collect();
            let absolute = path.canonicalize().unwrap();
            files.push((name.join("/"), absolute));
        }
    }
}
