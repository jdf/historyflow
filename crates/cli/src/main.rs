use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use historyflow_cli::serve::{self, Subject};
use historyflow_cli::{Error, git_file, wikipedia};
use historyflow_core::{Archive, Mode};

/// Shows how a document came to be, version by version: who wrote what,
/// and what became of it.
///
/// With no command, opens the viewer in your browser on what PATH names:
/// a git repository (any file's history, on request), a file in one, or
/// a history file. Everything is served from this machine; nothing is
/// fetched from anywhere else.
///
/// With --wikipedia, opens the viewer on an article's history instead,
/// which your browser fetches from the wiki. Add --output to fetch it
/// here and write it to a history file, opening nothing.
///
/// History files are written as archives (.hflow), which hold only what
/// each version changed, unless the name given ends in .json: then as a
/// bundle, plain JSON with every version's text written out.
#[derive(Parser)]
#[command(name = "historyflow", version, args_conflicts_with_subcommands = true)]
struct Cli {
    /// A git repository, a file in one, or a history file.
    #[arg(default_value = ".", conflicts_with = "wikipedia")]
    path: PathBuf,
    /// Show a Wikipedia article's history: its address
    /// (https://de.wikipedia.org/wiki/Schokolade), or just its title on
    /// the English Wikipedia.
    #[arg(long, value_name = "ARTICLE")]
    wikipedia: Option<String>,
    /// With --wikipedia: how many revisions to fetch, from the
    /// article's first.
    #[arg(long, value_name = "N", default_value_t = 500, requires = "wikipedia")]
    revisions: u32,
    /// With --wikipedia: how the wiki can reach you (an email address,
    /// a website, a wiki user name), which Wikipedia asks tools to
    /// say. Set it once instead with
    /// `git config --global historyflow.contact you@example.com`.
    #[arg(long, requires = "wikipedia")]
    contact: Option<String>,
    /// With --wikipedia: fetch the history now and write it to this
    /// file, instead of opening the viewer. Open the file later with
    /// `historyflow FILE`, and nothing is asked of the wiki again.
    #[arg(short, long, value_name = "FILE", requires = "wikipedia")]
    output: Option<PathBuf>,
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
        let contact = match &cli.wikipedia {
            Some(_) => wikipedia::contact(cli.contact.as_deref())?,
            None => String::new(),
        };
        if let (Some(article), Some(output)) = (&cli.wikipedia, &cli.output) {
            if cli.revisions == 0 {
                return Err("there is nothing to write in no revisions".into());
            }
            let get = wikipedia::rested(|url: &str| {
                wikipedia::curl(url, &contact, |seconds| {
                    eprint!("\rthe wiki asks for a pause of {seconds} seconds…");
                })
            });
            let archive = wikipedia::read(article, cli.revisions, get, |read| {
                // The spaces overwrite a longer line about a pause.
                eprint!("\r{read} revisions{:40}", "");
            });
            // End the line of progress, whatever came of it.
            eprintln!();
            let archive = archive?;
            if archive.is_empty() {
                return Err(
                    format!("{}: the wiki has no readable revisions", archive.title).into(),
                );
            }
            return write(&archive, output);
        }
        let subject = match &cli.wikipedia {
            Some(article) => Subject::wikipedia(article, cli.revisions, &contact)?,
            None => Subject::at(&cli.path, cli.first_parent)?,
        };
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
