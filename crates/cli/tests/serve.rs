//! The local server's answers, against a real repository.

mod common;

use common::repository;
use historyflow_cli::serve::{Files, Reply, Subject, reply};
use historyflow_core::{Archive, Mode};

const VIEWER: Files = &[
    ("index.html", b"<html><head><title>t</title></head></html>"),
    ("app.js", b"js"),
    ("pkg/historyflow_web_bg.wasm", b"wasm"),
];
const PORT: u16 = 8793;

fn get(subject: &Subject, url: &str) -> Reply {
    reply(subject, VIEWER, "GET", url, "127.0.0.1:8793", PORT)
}

fn text(reply: &Reply) -> &str {
    std::str::from_utf8(&reply.body).unwrap()
}

fn repo(dir: &tempfile::TempDir) -> Subject {
    Subject::at(dir.path(), false).unwrap()
}

#[test]
fn the_session_lists_the_repository_and_its_files() {
    let dir = repository();
    let session = get(&repo(&dir), "/api/session");
    assert_eq!(
        (session.status, session.content_type),
        (200, "application/json")
    );
    let session: serde_json::Value = serde_json::from_slice(&session.body).unwrap();
    assert_eq!(
        session["files"],
        serde_json::json!(["other.txt", "src/b.txt"])
    );
    assert_eq!(
        session["repository"],
        dir.path().file_name().unwrap().to_str().unwrap()
    );
    assert!(session["open"].is_null() && session["history"].is_null());
}

#[test]
fn a_file_named_on_the_command_line_is_the_one_to_open() {
    let dir = repository();
    let subject = Subject::at(&dir.path().join("src/b.txt"), false).unwrap();
    let session: serde_json::Value =
        serde_json::from_slice(&get(&subject, "/api/session").body).unwrap();
    assert_eq!(session["open"], "src/b.txt");
}

#[test]
fn a_files_history_is_read_from_git_on_request() {
    let dir = repository();
    let history = get(&repo(&dir), "/api/history?path=src%2Fb.txt");
    assert_eq!(
        (history.status, history.content_type),
        (200, "application/octet-stream")
    );
    let archive = Archive::from_bytes(&history.body).unwrap();
    assert!(archive.title.ends_with(": src/b.txt"));
    assert_eq!(archive.mode, Mode::Prose, ".txt is prose");
    let comments: Vec<_> = archive.versions().map(|v| v.comment).collect();
    assert_eq!(
        comments,
        ["Add a\n\nWith a body.", "Edit a", "Rename a to b"]
    );
}

#[test]
fn what_cannot_be_answered_is_refused_in_words() {
    let dir = repository();
    let subject = repo(&dir);
    let missing = get(&subject, "/api/history?path=nonesuch.rs");
    assert_eq!(missing.status, 404);
    assert!(text(&missing).contains("no history for nonesuch.rs"));
    // A path cannot reach outside the repository, or pass git an option.
    for path in ["../../etc/passwd", "--output=x", "%2Fetc%2Fpasswd"] {
        let outside = get(&subject, &format!("/api/history?path={path}"));
        assert_eq!(outside.status, 404, "{path}: {}", text(&outside));
    }
    assert_eq!(get(&subject, "/api/history").status, 400);
    assert_eq!(get(&subject, "/nonesuch.js").status, 404);
    assert_eq!(
        reply(&subject, VIEWER, "POST", "/", "127.0.0.1:8793", PORT).status,
        405
    );
}

#[test]
fn only_requests_addressed_to_this_machine_are_answered() {
    let dir = repository();
    let subject = repo(&dir);
    for host in ["localhost:8793", "127.0.0.1:8793", "[::1]:8793"] {
        assert_eq!(
            reply(&subject, VIEWER, "GET", "/api/session", host, PORT).status,
            200
        );
    }
    // Another site's name (pointed at this machine), another port, none.
    for host in ["evil.example:8793", "127.0.0.1:9999", "127.0.0.1", ""] {
        let refused = reply(&subject, VIEWER, "GET", "/api/session", host, PORT);
        assert_eq!(refused.status, 403, "{host}");
    }
}

#[test]
fn the_viewers_files_are_served_and_the_page_is_told_it_has_a_server() {
    let dir = repository();
    let subject = repo(&dir);
    for url in ["/", "/index.html", "/?file=src%2Fb.txt"] {
        let page = get(&subject, url);
        assert_eq!(
            (page.status, page.content_type),
            (200, "text/html; charset=utf-8")
        );
        assert!(
            text(&page).contains(r#"<meta name="historyflow-server" content="api/session" />"#)
        );
    }
    let script = get(&subject, "/app.js");
    assert_eq!(
        (script.content_type, text(&script)),
        ("text/javascript; charset=utf-8", "js")
    );
    assert_eq!(
        get(&subject, "/pkg/historyflow_web_bg.wasm").content_type,
        "application/wasm"
    );
    // A command built without the viewer says how to get one.
    let bare = reply(&subject, &[], "GET", "/", "127.0.0.1:8793", PORT);
    assert_eq!(bare.status, 500);
    assert!(text(&bare).contains("tools/build-cli.sh"));
}

#[test]
fn a_history_file_is_served_as_the_one_history() {
    let dir = repository();
    let mut archive = Archive::new("Saved", Mode::Prose);
    archive.push(&historyflow_core::SourceVersion {
        text: "Some text.".into(),
        ..Default::default()
    });
    for name in ["saved.hflow", "saved.json", "no-extension"] {
        let path = dir.path().join(name);
        historyflow_cli::write(&archive, &path).unwrap();
        if name == "no-extension" {
            // An archive is known by its content, whatever it is called.
            assert!(
                std::fs::read(&path)
                    .unwrap()
                    .starts_with(b"historyflow-archive")
            );
        }
        let subject = Subject::at(&path, false).unwrap();
        let session: serde_json::Value =
            serde_json::from_slice(&get(&subject, "/api/session").body).unwrap();
        assert_eq!(session["history"], "api/history");
        assert!(session["repository"].is_null());
        let served = Archive::from_bytes(&get(&subject, "/api/history").body).unwrap();
        assert_eq!(
            (served.title.as_str(), served.text(0)),
            ("Saved", "Some text.".into())
        );
    }
}

#[test]
fn a_directory_outside_any_repository_is_reported() {
    let dir = tempfile::tempdir().unwrap();
    assert!(Subject::at(dir.path(), false).is_err());
}
