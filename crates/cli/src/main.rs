use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use historyflow_cli::serve::{self, Subject};
use historyflow_cli::{Error, git_file};
use historyflow_core::{Archive, Mode};

/// Shows how a document came to be, version by version: who wrote what,
/// and what became of it.
///
/// With no command, opens the viewer in your browser on what PATH names:
/// a git repository (any file's history, on request), a file in one, or
/// a history file. Everything is served from this machine; nothing is
/// fetched from anywhere else.
///
/// History files are written as archives (.hflow), which hold only what
/// each version changed, unless the name given ends in .json: then as a
/// bundle, plain JSON with every version's text written out.
#[derive(Parser)]
#[command(name = "historyflow", version, args_conflicts_with_subcommands = true)]
struct Cli {
    /// A git repository, a file in one, or a history file.
    #[arg(default_value = ".")]
    path: PathBuf,
    /// The port to serve on; another is chosen if it is taken.
    #[arg(long, default_value_t = 8793)]
    port: u16,
    /// Print the address and do not open a browser.
    #[arg(long)]
    no_open: bool,
    /// Follow only the first parent of merges.
    #[arg(long)]
    first_parent: bool,
    #[command(subcommand)]
    command: Option<Command>,
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
    let Some(command) = cli.command else {
        let subject = Subject::at(&cli.path, cli.first_parent)?;
        return serve::serve(&subject, cli.port, |address| {
            eprintln!("history flow is at {address} — press Ctrl-C to stop");
            if !cli.no_open {
                open(address);
            }
        });
    };
    match command {
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

/// Opens an address in the browser, as best this system can. Failing is
/// harmless: the address has been printed.
fn open(address: &str) {
    let (program, args): (&str, &[&str]) = if cfg!(target_os = "macos") {
        ("open", &[])
    } else if cfg!(target_os = "windows") {
        ("cmd", &["/c", "start", ""])
    } else {
        ("xdg-open", &[])
    };
    let _ = std::process::Command::new(program)
        .args(args)
        .arg(address)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}
