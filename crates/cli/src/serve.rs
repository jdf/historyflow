//! Serves the viewer on this machine, with nothing fetched from anywhere
//! else: the page and its WebAssembly are part of the command, and what
//! the page shows comes from a git repository or a history file here.
//! The one exception is asked for by name: a Wikipedia article, whose
//! history the page then fetches from the wiki itself.
//!
//! The page learns what it is looking at from `api/session`, and asks
//! for one file's history at a time from `api/history?path=…`, which is
//! read from git when asked for.

use std::path::{Path, PathBuf};

use historyflow_core::{Archive, wikipedia};
use serde::Serialize;
use tiny_http::{Header, Method, Request, Server};

use crate::{Error, git_file};

include!(concat!(env!("OUT_DIR"), "/viewer.rs"));

/// The viewer's files, by path: the ones built into the command, or (in
/// tests) any others.
pub type Files = &'static [(&'static str, &'static [u8])];

/// What is being looked at.
pub enum Subject {
    /// A git working tree: any tracked file's history, on request.
    Repository {
        root: PathBuf,
        /// A file to show first, as a path within the tree.
        open: Option<String>,
        first_parent: bool,
    },
    /// One history, already read: the bytes of its archive.
    History(Vec<u8>),
    /// A Wikipedia article, for the page to fetch from the wiki.
    Wikipedia {
        /// Its title or address, as given.
        article: String,
        /// How many revisions to fetch, from the first.
        revisions: u32,
        /// How the wiki can reach whoever is running this.
        contact: String,
    },
}

impl Subject {
    /// What a path names: a history file, a file in a git working tree,
    /// or a directory in one.
    pub fn at(path: &Path, first_parent: bool) -> Result<Subject, Error> {
        if path.is_file() {
            if crate::is_history_file(path) {
                return Ok(Subject::History(crate::read(path)?.to_bytes()));
            }
            let (root, open) = git_file::locate(path)?;
            return Ok(Subject::Repository {
                root,
                open: Some(open),
                first_parent,
            });
        }
        Ok(Subject::Repository {
            root: git_file::root(path)?,
            open: None,
            first_parent,
        })
    }

    /// A Wikipedia article, by title or address, and how many of its
    /// revisions to fetch.
    pub fn wikipedia(article: &str, revisions: u32, contact: &str) -> Result<Subject, Error> {
        if wikipedia::Article::parse(article).is_none() {
            return Err(format!(
                "{article} is not an article's title, or its address on a Wikipedia \
                 (https://en.wikipedia.org/wiki/Chocolate)"
            )
            .into());
        }
        if revisions == 0 {
            return Err("there is nothing to show in no revisions".into());
        }
        Ok(Subject::Wikipedia {
            article: article.to_string(),
            revisions,
            contact: contact.to_string(),
        })
    }
}

/// What `api/session` tells the page.
#[derive(Serialize)]
struct Session<'a> {
    /// The repository's name, when looking at one.
    repository: Option<&'a str>,
    /// Its tracked files.
    files: Vec<String>,
    /// The file to show first.
    open: Option<&'a str>,
    /// Where to fetch the one history from, when looking at one.
    history: Option<&'a str>,
    /// The Wikipedia article to fetch, when looking at one.
    wikipedia: Option<Wikipedia<'a>>,
}

#[derive(Serialize)]
struct Wikipedia<'a> {
    article: &'a str,
    revisions: u32,
    contact: &'a str,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Reply {
    pub status: u16,
    pub content_type: &'static str,
    pub body: Vec<u8>,
}

impl Reply {
    fn ok(content_type: &'static str, body: impl Into<Vec<u8>>) -> Reply {
        Reply {
            status: 200,
            content_type,
            body: body.into(),
        }
    }

    fn error(status: u16, message: impl std::fmt::Display) -> Reply {
        Reply {
            status,
            content_type: "text/plain; charset=utf-8",
            body: message.to_string().into_bytes(),
        }
    }
}

/// Answers one request: `url` is its path and query, `host` its Host
/// header, and `port` the port being served on.
///
/// Only this machine is answered. The server listens only on the
/// loopback address, but a page on any site can make this machine's
/// browser ask it things; checking that the request was addressed to
/// this machine by name keeps such a page from reading the repository.
pub fn reply(
    subject: &Subject,
    viewer: Files,
    method: &str,
    url: &str,
    host: &str,
    port: u16,
) -> Reply {
    let local = ["localhost", "127.0.0.1", "[::1]"];
    if !local.iter().any(|name| host == format!("{name}:{port}")) {
        return Reply::error(403, "this server answers only its own machine");
    }
    if method != "GET" {
        return Reply::error(405, "only GET");
    }
    let (path, query) = url.split_once('?').unwrap_or((url, ""));
    match path {
        "/api/session" => session(subject),
        "/api/history" => history(subject, query),
        _ => file(viewer, path),
    }
}

fn session(subject: &Subject) -> Reply {
    let session = match subject {
        Subject::Repository { root, open, .. } => match git_file::files(root) {
            Ok(files) => Session {
                repository: root.file_name().and_then(|name| name.to_str()),
                files,
                open: open.as_deref(),
                history: None,
                wikipedia: None,
            },
            Err(error) => return Reply::error(500, error),
        },
        Subject::History(_) => Session {
            repository: None,
            files: Vec::new(),
            open: None,
            history: Some("api/history"),
            wikipedia: None,
        },
        Subject::Wikipedia {
            article,
            revisions,
            contact,
        } => Session {
            repository: None,
            files: Vec::new(),
            open: None,
            history: None,
            wikipedia: Some(Wikipedia {
                article,
                revisions: *revisions,
                contact,
            }),
        },
    };
    let json = serde_json::to_vec(&session).expect("serializable");
    Reply::ok("application/json", json)
}

fn history(subject: &Subject, query: &str) -> Reply {
    let archive = match subject {
        Subject::History(archive) => {
            return Reply::ok("application/octet-stream", archive.clone());
        }
        Subject::Wikipedia { .. } => {
            return Reply::error(404, "the page fetches this history from the wiki");
        }
        Subject::Repository {
            root, first_parent, ..
        } => {
            let path = query
                .split('&')
                .find_map(|pair| pair.strip_prefix("path="))
                .map(percent_decode);
            let Some(path) = path.filter(|path| !path.is_empty()) else {
                return Reply::error(400, "which file? api/history?path=…");
            };
            let options = git_file::Options {
                first_parent: *first_parent,
                mode: git_file::mode_for(&path),
            };
            match git_file::read_in(root, &path, &options) {
                Ok(bundle) => Archive::from_bundle(&bundle),
                Err(error) => return Reply::error(404, error),
            }
        }
    };
    Reply::ok("application/octet-stream", archive.to_bytes())
}

fn file(viewer: Files, path: &str) -> Reply {
    let name = match path.trim_start_matches('/') {
        "" => "index.html",
        name => name,
    };
    let Some((_, bytes)) = viewer.iter().find(|(file, _)| *file == name) else {
        return if viewer.is_empty() {
            Reply::error(
                500,
                "this command was built without the viewer; build it with tools/build-cli.sh",
            )
        } else {
            Reply::error(404, format!("no {name}"))
        };
    };
    let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
    let content_type = match extension {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "wasm" => "application/wasm",
        "json" => "application/json",
        _ => "application/octet-stream",
    };
    if name == "index.html" {
        // Tell the page it has a server to ask, which a copy of it on a
        // plain web host does not.
        let page = String::from_utf8_lossy(bytes);
        let marked = page.replacen(
            "<head>",
            "<head>\n    <meta name=\"historyflow-server\" content=\"api/session\" />",
            1,
        );
        return Reply::ok(content_type, marked);
    }
    Reply::ok(content_type, *bytes)
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let escaped = (bytes[i] == b'%')
            .then(|| text.get(i + 1..i + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(hex, 16).ok());
        match escaped {
            Some(byte) => {
                decoded.push(byte);
                i += 3;
            }
            None => {
                decoded.push(if bytes[i] == b'+' { b' ' } else { bytes[i] });
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Serves `subject` until the process is stopped. Listens on `port`, or
/// if that is taken (or 0) on any free port. Calls `started` with the
/// address once listening.
pub fn serve(subject: &Subject, port: u16, started: impl FnOnce(&str)) -> Result<(), Error> {
    let server = Server::http(("127.0.0.1", port))
        .or_else(|_| Server::http(("127.0.0.1", 0)))
        .map_err(|e| format!("cannot listen: {e}"))?;
    let port = server
        .server_addr()
        .to_ip()
        .map_or(port, |address| address.port());
    started(&format!("http://127.0.0.1:{port}/"));
    for request in server.incoming_requests() {
        answer(subject, request, port);
    }
    Ok(())
}

fn answer(subject: &Subject, request: Request, port: u16) {
    let host = request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Host"))
        .map(|header| header.value.to_string())
        .unwrap_or_default();
    let method = match request.method() {
        Method::Get => "GET",
        _ => "other",
    };
    let reply = reply(subject, VIEWER, method, request.url(), &host, port);
    let header = |name: &str, value: &str| Header::from_bytes(name, value).expect("a valid header");
    let response = tiny_http::Response::from_data(reply.body)
        .with_status_code(reply.status)
        .with_header(header("Content-Type", reply.content_type))
        // Every answer can change between requests: a new commit, a
        // rebuilt command.
        .with_header(header("Cache-Control", "no-store"));
    // The browser going away mid-answer is not this server's problem.
    let _ = request.respond(response);
}
