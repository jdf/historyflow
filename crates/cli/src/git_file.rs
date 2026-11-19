//! The history of one file in a git repository.
//!
//! Reads the repository through the `git` command, so it sees exactly
//! what `git log --follow` sees: the file is followed across renames,
//! and each commit that touched it becomes one version.
//!
//! git's history is a graph and the visualization is a line. By default
//! the commits come in the order `git log` lists them, which interleaves
//! the two sides of a merge by date; `first_parent` instead follows only
//! the mainline, where each version really is the successor of the one
//! before it and a merged branch's work appears at its merge commit.

use std::ffi::OsStr;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use historyflow_core::{Bundle, Mode, SourceVersion};

#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Follow only the first parent of each merge.
    pub first_parent: bool,
    pub mode: Mode,
}

pub use crate::Error;

/// How a file's versions are best compared, going by its name: prose by
/// words, everything else by lines.
pub fn mode_for(path: &str) -> Mode {
    let extension = path.rsplit_once('.').map_or("", |(_, extension)| extension);
    match extension.to_ascii_lowercase().as_str() {
        "md" | "markdown" | "txt" | "rst" | "adoc" | "tex" | "org" => Mode::Prose,
        _ => Mode::Lines,
    }
}

/// Separates commits, and fields within a commit, in `git log` output.
/// Neither can occur in a commit message git accepts as text.
const RECORD: char = '\x1e';
const FIELD: char = '\x1f';

/// Reads the history of `file`, a path to a file inside a git working
/// tree (it need not still exist, if its directory does).
pub fn read(file: &Path, options: &Options) -> Result<Bundle, Error> {
    let (root, path) = locate(file)?;
    read_in(&root, &path, options)
}

/// The working tree a file is in, and the file's path within it.
pub fn locate(file: &Path) -> Result<(PathBuf, String), Error> {
    let directory = match file.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let name = file
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or("the path does not name a file")?;
    let prefix = git(directory, &["rev-parse", "--show-prefix"])?;
    Ok((root(directory)?, format!("{}{name}", prefix.trim_end())))
}

/// The top of the working tree `directory` is in.
pub fn root(directory: &Path) -> Result<PathBuf, Error> {
    let top = git(directory, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(top.trim_end()))
}

/// The files git tracks in the working tree at `root` that have a
/// history worth reading as text, as paths within the tree, in git's
/// order. Left out: what git takes to be binary, what is not a file
/// (a symbolic link, a submodule), and anything whose name, or the name
/// of a directory it is in, starts with a dot.
pub fn files(root: &Path) -> Result<Vec<String>, Error> {
    // Each entry is "i/<what is committed> w/<in the tree> attr/<…>",
    // a tab, and the path. What is committed is "-text" for binary
    // content and empty for what is not a file.
    let listed = git(root, &["ls-files", "--eol", "-z"])?;
    Ok(listed
        .split_terminator('\0')
        .filter_map(|entry| entry.split_once('\t'))
        .filter(|(kind, path)| {
            let committed = kind.split_whitespace().next().unwrap_or("");
            committed != "i/-text"
                && committed != "i/"
                && !path.split('/').any(|name| name.starts_with('.'))
        })
        .map(|(_, path)| path.to_string())
        .collect())
}

/// Reads the history of the file at `path` within the working tree at
/// `root`.
pub fn read_in(root: &Path, path: &str, options: &Options) -> Result<Bundle, Error> {
    let commits = log(root, path, options)?;
    if commits.is_empty() {
        return Err(format!("git has no history for {path}").into());
    }
    let texts = contents(root, &commits)?;
    let versions = commits
        .into_iter()
        .zip(texts)
        .map(|(commit, text)| SourceVersion {
            id: commit.hash,
            author: commit.author,
            anonymous: false,
            timestamp: commit.timestamp,
            comment: commit.message,
            text,
        })
        .collect();
    let repository = root.file_name().and_then(OsStr::to_str).unwrap_or("");
    Ok(Bundle::new(
        format!("{repository}: {path}"),
        options.mode,
        versions,
    ))
}

struct Commit {
    hash: String,
    author: String,
    timestamp: i64,
    message: String,
    /// The file's path in this commit, or `None` if the commit deleted it.
    path: Option<String>,
}

/// The commits that touched the file, oldest first.
fn log(root: &Path, path: &str, options: &Options) -> Result<Vec<Commit>, Error> {
    let format = format!("--format={RECORD}%H{FIELD}%an{FIELD}%at{FIELD}%B{FIELD}");
    let mut args = vec!["log", "--follow", "--name-status", "-z", &format];
    if options.first_parent {
        args.push("--first-parent");
    }
    args.extend(["--", path]);
    let output = git(root, &args)?;

    // Newest first. A commit's own name-status entry gives the file's
    // path there; one with no entry (a merge) has the path of the commit
    // after it, which is `current`.
    let mut current = path.to_string();
    let mut commits = Vec::new();
    for record in output.split(RECORD).skip(1) {
        let fields: Vec<&str> = record.split(FIELD).collect();
        let &[hash, author, timestamp, message, changes] = fields.as_slice() else {
            return Err(format!("unexpected git log output: {record:?}").into());
        };
        // With -z: status, path — or for a rename or copy: status, old
        // path, new path.
        let changes: Vec<&str> = changes
            .split('\0')
            .filter(|s| !s.trim().is_empty())
            .collect();
        let path = match changes.as_slice() {
            [status, _] if status.trim() == "D" => None,
            [_, path] => Some(path.to_string()),
            [_, old, new] => {
                let path = Some(new.to_string());
                current = old.to_string();
                commits.push(commit(hash, author, timestamp, message, path)?);
                continue;
            }
            _ => Some(current.clone()),
        };
        if let Some(path) = &path {
            current = path.clone();
        }
        commits.push(commit(hash, author, timestamp, message, path)?);
    }
    commits.reverse();
    Ok(commits)
}

fn commit(
    hash: &str,
    author: &str,
    timestamp: &str,
    message: &str,
    path: Option<String>,
) -> Result<Commit, Error> {
    Ok(Commit {
        hash: hash.into(),
        author: author.into(),
        timestamp: timestamp.parse()?,
        message: message.trim_end().into(),
        path,
    })
}

/// The file's text in each commit, through one `git cat-file --batch`.
/// A commit that deleted the file has empty text; bytes that are not
/// UTF-8 are replaced.
fn contents(root: &Path, commits: &[Commit]) -> Result<Vec<String>, Error> {
    let mut child = base(root)
        .args(["cat-file", "--batch"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().expect("piped");
    let requests: String = commits
        .iter()
        .filter_map(|c| c.path.as_ref().map(|path| format!("{}:{path}\n", c.hash)))
        .collect();
    // Written from another thread: git answers as it reads, and would
    // block on a full output pipe while we were still writing.
    let writer = std::thread::spawn(move || stdin.write_all(requests.as_bytes()));

    let mut stdout = BufReader::new(child.stdout.take().expect("piped"));
    let mut texts = Vec::with_capacity(commits.len());
    for commit in commits {
        if commit.path.is_none() {
            texts.push(String::new());
            continue;
        }
        // "<oid> blob <size>", then that many bytes and a newline.
        let mut header = String::new();
        stdout.read_line(&mut header)?;
        let size: usize = match header.split_whitespace().collect::<Vec<_>>().as_slice() {
            [_, "blob", size] => size.parse()?,
            _ => return Err(format!("git cat-file: {}", header.trim_end()).into()),
        };
        let mut bytes = vec![0; size + 1];
        stdout.read_exact(&mut bytes)?;
        bytes.pop();
        texts.push(String::from_utf8_lossy(&bytes).into_owned());
    }
    writer.join().expect("writer thread")?;
    if !child.wait()?.success() {
        return Err("git cat-file failed".into());
    }
    Ok(texts)
}

/// A `git` command in `directory` that the user's configuration cannot
/// change the output of.
fn base(directory: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "core.quotePath=false",
            "-c",
            "log.showSignature=false",
        ])
        .arg("--no-pager");
    command
}

fn git(directory: &Path, args: &[&str]) -> Result<String, Error> {
    let output = base(directory).args(args).output()?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git {}: {}", args[0], message.trim()).into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}
