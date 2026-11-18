//! A rough scale check: builds a synthetic history of a large page with
//! many small edits and occasional blanking and repair, and reports how
//! long attribution and link extraction take.
//!
//!   cargo run --release -p historyflow-core --example scale [versions] [words]

use std::time::Instant;

use historyflow_core::{Config, HistoryBuilder, SourceVersion};

fn main() {
    let mut args = std::env::args()
        .skip(1)
        .map(|a| a.parse::<usize>().unwrap());
    let versions = args.next().unwrap_or(2000);
    let words = args.next().unwrap_or(8000);

    let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
    let mut random = move |n: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % n as u64) as usize
    };
    let word = |random: &mut dyn FnMut(usize) -> usize| match random(12) {
        0 => ".".to_string(),
        1 => "\n\n".to_string(),
        _ => format!("w{}", random(5000)),
    };
    let mut doc: Vec<String> = (0..words).map(|_| word(&mut random)).collect();

    for (name, config) in [
        ("prose", Config::prose as fn() -> Config),
        ("sentences_2004", Config::sentences_2004),
        ("source_lines", Config::source_lines),
    ] {
        let mut builder = HistoryBuilder::new(config());
        let mut bytes = 0;
        let start = Instant::now();
        for v in 0..versions {
            let text = if v % 50 == 49 {
                "blanked".to_string()
            } else {
                let at = random(doc.len());
                let end = (at + random(20)).min(doc.len());
                let added: Vec<String> = (0..random(30)).map(|_| word(&mut random)).collect();
                doc.splice(at..end, added);
                doc.join(" ")
            };
            bytes += text.len();
            builder.push(&SourceVersion {
                text,
                ..Default::default()
            });
        }
        let built = start.elapsed();
        let history = builder.finish();
        let start = Instant::now();
        let links: usize = (1..versions as u32)
            .map(|v| history.links(v - 1, v).len())
            .sum();
        let runs: usize = history.versions().iter().map(|v| v.runs().len()).sum();
        println!(
            "{name}: {versions} versions, {} MB of text, {} tokens, {runs} runs, {links} links; \
             build {built:.2?}, links {:.2?}",
            bytes / 1_000_000,
            history.token_count(),
            start.elapsed(),
        );
    }
}
