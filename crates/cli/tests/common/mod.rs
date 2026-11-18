//! A real git repository built for a test.

use std::path::Path;
use std::process::Command;

/// Runs git in `repo`, isolated from the machine's configuration, as
/// `author` at `time` (seconds since the epoch).
pub fn git(repo: &Path, author: &str, time: i64, args: &[&str]) {
    let date = format!("@{time} +0000");
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", author)
        .env("GIT_AUTHOR_EMAIL", "author@example.com")
        .env("GIT_AUTHOR_DATE", &date)
        .env("GIT_COMMITTER_NAME", author)
        .env("GIT_COMMITTER_EMAIL", "author@example.com")
        .env("GIT_COMMITTER_DATE", &date)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn commit(repo: &Path, author: &str, time: i64, message: &str) {
    git(repo, author, time, &["add", "-A"]);
    git(repo, author, time, &["commit", "-q", "-m", message]);
}

/// ada writes src/a.txt, bob edits it, ada renames it to src/b.txt with a
/// small edit, and an unrelated file changes in between.
pub fn repository() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git(repo, "ada", 0, &["init", "-q", "-b", "main"]);
    std::fs::create_dir(repo.join("src")).unwrap();
    let body = "one\ntwo\nthree\nfour\nfive\nsix\nseven\neight\n";
    std::fs::write(repo.join("src/a.txt"), body).unwrap();
    commit(repo, "ada", 1000, "Add a\n\nWith a body.");
    std::fs::write(repo.join("other.txt"), "unrelated\n").unwrap();
    commit(repo, "bob", 2000, "Unrelated");
    std::fs::write(repo.join("src/a.txt"), body.replace("two", "2")).unwrap();
    commit(repo, "bob", 3000, "Edit a");
    std::fs::rename(repo.join("src/a.txt"), repo.join("src/b.txt")).unwrap();
    std::fs::write(repo.join("src/b.txt"), body.replace("two", "2") + "nine\n").unwrap();
    commit(repo, "ada", 4000, "Rename a to b");
    dir
}
