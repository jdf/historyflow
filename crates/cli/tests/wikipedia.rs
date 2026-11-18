//! Fetching a Wikipedia article into an archive, with the wiki's answers
//! supplied by the test.

use historyflow_cli::wikipedia::{contact, read};

#[test]
fn a_contact_given_is_the_one_used_if_it_will_do() {
    assert_eq!(
        contact(Some(" ada@example.com ")).unwrap(),
        "ada@example.com"
    );
    assert!(contact(Some("ada) other/1.0 (x")).is_err());
    assert!(contact(Some("two\nlines")).is_err());
}

fn response(ids: std::ops::Range<u32>, more: bool) -> String {
    let revisions: Vec<_> = ids
        .map(|id| {
            serde_json::json!({
                "revid": id,
                "user": if id % 2 == 0 { "Ada" } else { "Brooke" },
                "timestamp": format!("2003-01-{:02}T00:00:00Z", id + 1),
                "comment": format!("edit {id}"),
                "slots": { "main": { "content": format!("Chocolate, revision {id}.") } },
            })
        })
        .collect();
    let mut response = serde_json::json!({
        "query": { "pages": [{ "title": "Chocolate", "revisions": revisions }] },
    });
    if more {
        response["continue"] = serde_json::json!({ "rvcontinue": "20030101|9", "continue": "||" });
    }
    response.to_string()
}

#[test]
fn batches_are_fetched_until_enough_revisions_are_read() {
    let mut asked = Vec::new();
    let mut progress = Vec::new();
    let archive = read(
        "https://en.wikipedia.org/wiki/chocolate",
        70,
        |url| {
            asked.push(url.to_string());
            Ok(if asked.len() == 1 {
                response(0..50, true)
            } else {
                response(50..70, true)
            })
        },
        |read| progress.push(read),
    )
    .unwrap();

    assert_eq!((archive.len(), archive.title.as_str()), (70, "Chocolate"));
    assert_eq!(archive.version(69).text, "Chocolate, revision 69.");
    assert_eq!(archive.version(1).author, "Brooke");
    assert_eq!(progress, [50, 70]);
    assert_eq!(asked.len(), 2);
    assert!(asked[0].contains("rvlimit=50") && !asked[0].contains("rvcontinue"));
    assert!(asked[1].contains("rvlimit=20") && asked[1].contains("rvcontinue=20030101"));
}

#[test]
fn a_short_article_ends_where_the_wiki_says() {
    let mut asked = 0;
    let archive = read(
        "Chocolate",
        500,
        |_| {
            asked += 1;
            Ok(response(0..3, false))
        },
        |_| {},
    )
    .unwrap();
    assert_eq!((archive.len(), asked), (3, 1));
}

#[test]
fn failures_are_reported() {
    let no_fetch = |_: &str| -> Result<String, historyflow_cli::Error> { panic!("nothing to ask") };
    assert!(read("https://example.com/x", 5, no_fetch, |_| {}).is_err());
    let offline = read("Chocolate", 5, |_| Err("no network".into()), |_| {});
    assert_eq!(offline.unwrap_err().to_string(), "no network");
    let missing = r#"{"query":{"pages":[{"title":"Nonesuch","missing":true}]}}"#;
    let error = read("Nonesuch", 5, |_| Ok(missing.to_string()), |_| {}).unwrap_err();
    assert!(error.to_string().contains("Nonesuch"), "{error}");
}
