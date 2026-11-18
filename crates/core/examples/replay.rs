//! Times the steps of loading a history file: reading the archive,
//! making every version's text, and attributing it in each mode.
//!
//!   cargo run --release -p historyflow-core --example replay <file>

use std::time::Instant;

use historyflow_core::{Archive, HistoryBuilder, Mode, Spacing, layout};

fn main() {
    let path = std::env::args().nth(1).expect("a history file");
    let bytes = std::fs::read(&path).expect("readable");

    let start = Instant::now();
    let archive = Archive::read(&bytes).expect("a history file");
    println!(
        "{}: {} versions, {} bytes of text in a file of {} bytes; read in {:.2?}",
        archive.title,
        archive.len(),
        archive.text_bytes(),
        bytes.len(),
        start.elapsed()
    );

    let start = Instant::now();
    let versions: Vec<_> = archive.versions().collect();
    println!("every version's text made in {:.2?}", start.elapsed());

    for mode in [Mode::Prose, Mode::Sentences, Mode::Lines] {
        let start = Instant::now();
        let mut builder = HistoryBuilder::new(mode.config());
        for version in &versions {
            builder.push(version);
        }
        let history = builder.finish();
        let attributed = start.elapsed();
        let start = Instant::now();
        let quads = layout(&history, Spacing::Version).quads.len();
        println!(
            "{}: attributed in {attributed:.2?} ({} tokens), laid out in {:.2?} ({quads} quads)",
            mode.name(),
            history.token_count(),
            start.elapsed(),
        );
    }

    let start = Instant::now();
    let mut again = Archive::new(archive.title.clone(), archive.mode);
    for version in &versions {
        again.push(version);
    }
    println!(
        "archived afresh in {:.2?} ({} bytes)",
        start.elapsed(),
        again.to_bytes().len()
    );
}
