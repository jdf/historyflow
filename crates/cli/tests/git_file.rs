//! Runs the git-file source against a real repository built for the test.

use historyflow_cli::git_file::{self, Options};
use historyflow_core::Mode;

mod common;

use common::{commit, repository};

#[test]
fn follows_one_file_across_a_rename() {
    let dir = repository();
    let bundle = git_file::read(&dir.path().join("src/b.txt"), &Options::default()).unwrap();

    assert_eq!(bundle.format, "historyflow-bundle/1");
    assert!(bundle.title.ends_with(": src/b.txt"), "{}", bundle.title);
    assert_eq!(bundle.mode, Mode::Lines);
    let summary: Vec<_> = bundle
        .versions
        .iter()
        .map(|v| {
            (
                v.author.as_str(),
                v.timestamp,
                v.comment.as_str(),
                v.text.lines().count(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        [
            ("ada", 1000, "Add a\n\nWith a body.", 8),
            ("bob", 3000, "Edit a", 8),
            ("ada", 4000, "Rename a to b", 9),
        ]
    );
    assert!(bundle.versions[1].text.starts_with("one\n2\nthree\n"));
    assert!(
        bundle
            .versions
            .iter()
            .all(|v| v.id.len() == 40 && !v.anonymous)
    );
}

#[test]
fn a_deletion_is_an_empty_version() {
    let dir = repository();
    let repo = dir.path();
    std::fs::remove_file(repo.join("src/b.txt")).unwrap();
    commit(repo, "bob", 5000, "Delete b");
    let bundle = git_file::read(&repo.join("src/b.txt"), &Options::default()).unwrap();
    let last = bundle.versions.last().unwrap();
    assert_eq!(
        (last.comment.as_str(), last.text.as_str()),
        ("Delete b", "")
    );
    assert_eq!(bundle.versions.len(), 4);
}

#[test]
fn reports_a_file_with_no_history() {
    let dir = repository();
    let error = git_file::read(&dir.path().join("src/missing.txt"), &Options::default());
    assert!(error.unwrap_err().to_string().contains("no history"));
}

#[test]
fn the_bundle_round_trips_through_json() {
    let dir = repository();
    let bundle = git_file::read(&dir.path().join("src/b.txt"), &Options::default()).unwrap();
    let json = serde_json::to_string(&bundle).unwrap();
    assert_eq!(
        serde_json::from_str::<historyflow_core::Bundle>(&json).unwrap(),
        bundle
    );
}

#[test]
fn a_history_is_written_as_an_archive_and_read_from_either_kind_of_file() {
    let dir = repository();
    let bundle = git_file::read(&dir.path().join("src/b.txt"), &Options::default()).unwrap();
    let archive = historyflow_core::Archive::from_bundle(&bundle);
    let packed = dir.path().join("out.hflow");
    // The name does not make it anything but an archive.
    let misnamed = dir.path().join("out.json");
    let scripted = dir.path().join("scripted.json");
    historyflow_cli::write(&archive, &packed).unwrap();
    historyflow_cli::write(&archive, &misnamed).unwrap();
    std::fs::write(&scripted, serde_json::to_vec(&bundle).unwrap()).unwrap();

    for path in [&packed, &misnamed, &scripted] {
        assert!(historyflow_cli::is_history_file(path));
        assert_eq!(historyflow_cli::read(path).unwrap().to_bundle(), bundle);
    }
    for path in [&packed, &misnamed] {
        let written = std::fs::read(path).unwrap();
        assert!(written.starts_with(b"historyflow-archive/1\n"));
    }
    assert!(!historyflow_cli::is_history_file(
        &dir.path().join("src/b.txt")
    ));
    let missing = historyflow_cli::read(&dir.path().join("nonesuch")).unwrap_err();
    assert!(missing.to_string().contains("nonesuch"));
}

#[test]
fn prose_is_told_from_source_by_its_name() {
    use historyflow_core::Mode;
    assert_eq!(git_file::mode_for("notes/README.MD"), Mode::Prose);
    assert_eq!(git_file::mode_for("src/main.rs"), Mode::Lines);
    assert_eq!(git_file::mode_for("Makefile"), Mode::Lines);
}
