use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use historyflow_cli::{Error, git_file};
use historyflow_core::{Archive, Mode};

/// Turns a document's history into a file the history flow page loads.
///
/// Files are written as archives, which hold only what each version
/// changed, unless the name given ends in .json: then as a bundle, plain
/// JSON with every version's text written out.
#[derive(Parser)]
#[command(name = "historyflow", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// The history of one file in a git repository, followed across
    /// renames.
    GitFile {
        /// The file, inside a git working tree.
        file: PathBuf,
        /// Where to write the history; as JSON to standard output if
        /// omitted.
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
    /// Rewrites a history file as the other kind: a bundle (.json) as an
    /// archive, or an archive as a bundle.
    Convert {
        /// An archive or a bundle.
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
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

fn run(cli: Cli) -> Result<(), Error> {
    match cli.command {
        Command::GitFile {
            file,
            output,
            mode,
            first_parent,
            max_versions,
        } => {
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
            match output {
                Some(path) => write(&Archive::from_bundle(&bundle), &path),
                None => Ok(std::io::stdout().write_all(&serde_json::to_vec(&bundle)?)?),
            }
        }
        Command::Convert { input, output } => write(&historyflow_cli::read(&input)?, &output),
    }
}

/// Writes the history and says what was written.
fn write(archive: &Archive, path: &Path) -> Result<(), Error> {
    let written = historyflow_cli::write(archive, path)?;
    eprintln!(
        "{}: {} versions, {} bytes of text → {} ({written} bytes)",
        archive.title,
        archive.len(),
        archive.text_bytes(),
        path.display(),
    );
    Ok(())
}
