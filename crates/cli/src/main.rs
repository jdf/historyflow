use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use historyflow_cli::serve::{self, Subject};
use historyflow_cli::{Error, git_file, wikipedia};
use historyflow_core::Archive;

/// Shows how a document came to be, version by version: who wrote what,
/// and what became of it.
///
/// Opens the viewer in your browser on what WHAT names: a git repository
/// (any file's history, on request), a file in one, a history file, or
/// the address of a Wikipedia article. Everything is served from this
/// machine, and nothing is fetched from anywhere else, except an
/// article's history from its wiki.
///
/// With --output, opens nothing: the history is written to a history
/// file (.hflow), which holds only what each version changed and opens
/// later without git or the wiki.
#[derive(Parser)]
#[command(name = "historyflow", version)]
struct Cli {
    /// A git repository, a file in one, a history file, or a Wikipedia
    /// article's address (https://en.wikipedia.org/wiki/Chocolate).
    #[arg(default_value = ".")]
    what: String,
    /// Write the history to this file instead of opening the viewer.
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
    /// For an article: how many revisions to fetch, from its first.
    /// 500 unless given.
    #[arg(long, value_name = "N")]
    revisions: Option<u32>,
    /// For an article: how the wiki can reach you (an email address, a
    /// website, a wiki user name), which Wikipedia asks tools to say.
    /// Set it once instead with
    /// `git config --global historyflow.contact you@example.com`.
    #[arg(long)]
    contact: Option<String>,
    /// For git: follow only the first parent of merges, so that a
    /// merged branch appears as one version, at its merge.
    #[arg(long)]
    first_parent: bool,
    /// The port to serve on; another is chosen if it is taken.
    #[arg(long, default_value_t = 8793)]
    port: u16,
    /// Print the address and do not open a browser.
    #[arg(long)]
    no_open: bool,
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
    // An address is an article; anything else is a path.
    let article = ["https://", "http://"]
        .iter()
        .any(|scheme| cli.what.starts_with(scheme));
    let subject = if article {
        let revisions = cli.revisions.unwrap_or(500);
        let contact = wikipedia::contact(cli.contact.as_deref())?;
        // Checks the address and the number before anything is fetched.
        let subject = Subject::wikipedia(&cli.what, revisions, &contact)?;
        if let Some(output) = &cli.output {
            return write(&fetch(&cli.what, revisions, &contact)?, output);
        }
        subject
    } else {
        if cli.revisions.is_some() || cli.contact.is_some() {
            return Err("--revisions and --contact are for a Wikipedia article's address".into());
        }
        let path = Path::new(&cli.what);
        if let Some(output) = &cli.output {
            return write(&history(path, cli.first_parent)?, output);
        }
        Subject::at(path, cli.first_parent)?
    };
    serve::serve(&subject, cli.port, |address| {
        eprintln!("history flow is at {address} — press Ctrl-C to stop");
        if !cli.no_open {
            open(address);
        }
    })
}

/// The one history a path names, to write out: a history file's own, or
/// that of a file in a git repository.
fn history(path: &Path, first_parent: bool) -> Result<Archive, Error> {
    if path.is_dir() {
        return Err(format!(
            "{} is a directory: name the file whose history to write",
            path.display()
        )
        .into());
    }
    if historyflow_cli::is_history_file(path) {
        return historyflow_cli::read(path);
    }
    let options = git_file::Options {
        first_parent,
        mode: git_file::mode_for(&path.to_string_lossy()),
    };
    Ok(Archive::from_bundle(&git_file::read(path, &options)?))
}

/// Fetches an article's history from its wiki, saying how it is going.
fn fetch(article: &str, revisions: u32, contact: &str) -> Result<Archive, Error> {
    let client = wikipedia::Client::new(contact);
    let get = wikipedia::rested(|url: &str| {
        client.get(url, |seconds| {
            eprint!("\rthe wiki asks for a pause of {seconds} seconds…");
        })
    });
    let archive = wikipedia::read(article, revisions, get, |read| {
        // The spaces overwrite a longer line about a pause.
        eprint!("\r{read} revisions{:40}", "");
    });
    // End the line of progress, whatever came of it.
    eprintln!();
    let archive = archive?;
    if archive.is_empty() {
        return Err(format!("{}: the wiki has no readable revisions", archive.title).into());
    }
    Ok(archive)
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
