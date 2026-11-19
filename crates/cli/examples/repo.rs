//! Measures what a history file of a whole git repository would weigh:
//! every listed (text, not hidden) file's history, as archives.
//!
//!   cargo run --release -p historyflow-cli --example repo -- [--cap BYTES] REPO...
//!
//! With --cap, a file whose newest version is longer than that is left
//! out: a few generated files (a disassembly, an approved SVG) can
//! outweigh everything written by hand.
//!
//! For each repository, two ways of storing it are sized:
//!
//!   separate  one archive per file, as `historyflow FILE -o` writes
//!             them now, added up
//!   together  one archive holding every file's versions end to end, so
//!             that the files share one table of chunks and one deflate
//!             stream (a stand-in for a real repository format, which
//!             would also store each commit's details once)

use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use historyflow_cli::git_file;
use historyflow_core::{Archive, Bundle, Mode};

fn main() {
    println!(
        "{:<22} {:>7} {:>6} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>7}",
        "repository",
        "commits",
        "files",
        "versions",
        "tree text",
        "all text",
        "separate",
        "together",
        "git",
        "seconds"
    );
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut cap = usize::MAX;
    if args.first().is_some_and(|arg| arg == "--cap") {
        cap = args[1].parse().expect("--cap BYTES");
        args.drain(..2);
    }
    for repo in args {
        if let Err(error) = measure(Path::new(&repo), cap) {
            eprintln!("{repo}: {error}");
        }
    }
}

fn measure(repo: &Path, cap: usize) -> Result<(), historyflow_cli::Error> {
    let started = Instant::now();
    let root = git_file::root(repo)?;
    let files = git_file::files(&root)?;
    let next = AtomicUsize::new(0);
    let read: Mutex<Vec<(usize, Bundle)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..std::thread::available_parallelism().map_or(4, |n| n.get()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(path) = files.get(index) else { break };
                    if index.is_multiple_of(50) {
                        eprint!("\r{index} of {} files", files.len());
                    }
                    let options = git_file::Options {
                        first_parent: false,
                        mode: git_file::mode_for(path),
                    };
                    // A file git lists but has no history for is skipped.
                    if let Ok(bundle) = git_file::read_in(&root, path, &options) {
                        read.lock().unwrap().push((index, bundle));
                    }
                }
            });
        }
    });
    eprint!("\r{:40}\r", "");
    let mut read = read.into_inner().unwrap();
    read.sort_by_key(|(index, _)| *index);
    read.retain(|(_, bundle)| bundle.versions.last().is_some_and(|v| v.text.len() <= cap));

    let mut together = Archive::new("", Mode::Lines);
    let (mut separate, mut versions, mut all_text, mut tree_text) = (0, 0, 0, 0);
    for (_, bundle) in &read {
        let archive = Archive::from_bundle(bundle);
        separate += archive.to_bytes().len();
        versions += archive.len();
        all_text += archive.text_bytes();
        tree_text += bundle.versions.last().map_or(0, |v| v.text.len());
        for version in &bundle.versions {
            together.push(version);
        }
    }
    let together = together.to_bytes().len();

    // With TOP=N in the environment, the N heaviest files.
    if let Some(top) = std::env::var("TOP").ok().and_then(|n| n.parse().ok()) {
        let mut heaviest: Vec<_> = read
            .iter()
            .map(|(index, bundle)| {
                let archive = Archive::from_bundle(bundle);
                (
                    archive.to_bytes().len(),
                    archive.len(),
                    archive.text_bytes(),
                    &files[*index],
                )
            })
            .collect();
        heaviest.sort_by_key(|entry| std::cmp::Reverse(entry.0));
        for (bytes, versions, text, path) in heaviest.into_iter().take(top) {
            println!(
                "    {:>10} {versions:>5} versions {:>10} of text  {path}",
                size(bytes as u64),
                size(text)
            );
        }
    }

    let git = |args: &[&str]| -> String {
        let output = Command::new("git").current_dir(&root).args(args).output();
        output.map_or(String::new(), |o| {
            String::from_utf8_lossy(&o.stdout).into_owned()
        })
    };
    let commits = git(&["rev-list", "--count", "HEAD"]);
    // Loose and packed objects, in KiB.
    let objects: u64 = git(&["count-objects", "-v"])
        .lines()
        .filter_map(|line| line.split_once(": "))
        .filter(|(name, _)| *name == "size" || *name == "size-pack")
        .filter_map(|(_, size)| size.trim().parse::<u64>().ok())
        .sum();

    println!(
        "{:<22} {:>7} {:>6} {:>8} {:>10} {:>10} {:>10} {:>10} {:>10} {:>7.1}",
        root.file_name().and_then(|n| n.to_str()).unwrap_or("?"),
        commits.trim(),
        read.len(),
        versions,
        size(tree_text as u64),
        size(all_text),
        size(separate as u64),
        size(together as u64),
        size(objects * 1024),
        started.elapsed().as_secs_f64(),
    );
    Ok(())
}

fn size(bytes: u64) -> String {
    match bytes {
        0..1_000 => format!("{bytes} B"),
        1_000..1_000_000 => format!("{:.1} KB", bytes as f64 / 1e3),
        1_000_000..1_000_000_000 => format!("{:.2} MB", bytes as f64 / 1e6),
        _ => format!("{:.2} GB", bytes as f64 / 1e9),
    }
}
