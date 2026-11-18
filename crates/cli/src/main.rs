use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use historyflow_cli::git_file;
use historyflow_core::Mode;

/// Turns a document's history into a bundle the history flow page loads.
#[derive(Parser)]
#[command(name = "historyflow", version)]
struct Cli {
    #[command(subcommand)]
    source: Source,
}

#[derive(Subcommand)]
enum Source {
    /// The history of one file in a git repository, followed across
    /// renames.
    GitFile {
        /// The file, inside a git working tree.
        file: PathBuf,
        /// Where to write the bundle; standard output if omitted.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// How the page should compare versions unless told otherwise.
        #[arg(long, value_enum, default_value = "lines")]
        mode: ModeArg,
        /// Follow only the first parent of merges: a merged branch
        /// appears as one version, at its merge.
        #[arg(long)]
        first_parent: bool,
        /// Keep only the most recent N versions.
        #[arg(long, value_name = "N")]
        max_versions: Option<usize>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ModeArg {
    /// Whole lines, each version diffed against the one before.
    Lines,
    /// Words, matched across the whole history.
    Prose,
    /// Sentences, as in the original history flow.
    Sentences,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("historyflow: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), git_file::Error> {
    let Source::GitFile {
        file,
        output,
        mode,
        first_parent,
        max_versions,
    } = cli.source;
    let options = git_file::Options {
        first_parent,
        max_versions,
        mode: match mode {
            ModeArg::Lines => Mode::Lines,
            ModeArg::Prose => Mode::Prose,
            ModeArg::Sentences => Mode::Sentences,
        },
    };
    let bundle = git_file::read(&file, &options)?;
    let json = serde_json::to_string(&bundle)?;
    match output {
        Some(path) => {
            std::fs::write(&path, json)?;
            eprintln!(
                "{}: {} versions → {}",
                bundle.title,
                bundle.versions.len(),
                path.display()
            );
        }
        None => std::io::stdout().write_all(json.as_bytes())?,
    }
    Ok(())
}
