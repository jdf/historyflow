//! The edit history of a wiki article, through the MediaWiki Action API.
//!
//! This module is the half of the source that needs no network: it
//! builds the request for the next batch of revisions and reads the
//! response. Whoever has a network (the page's `fetch`) passes the one to
//! the server and the other back. Revisions come oldest first, at most
//! [`BATCH`] to a request, each response saying how to ask for the next.

use serde::Deserialize;

use crate::model::SourceVersion;

/// How this tool names itself to a wiki, as Wikimedia's User-Agent
/// policy asks: its name and version and, in parentheses, how to reach
/// whoever is running it (`contact`: an email address, a website, a
/// wiki user name), when that is known. The page sends it as
/// `Api-User-Agent`, since a page cannot choose its `User-Agent`.
pub fn user_agent(contact: &str) -> String {
    let tool = format!("historyflow/{}", crate::version());
    match contact.trim() {
        "" => tool,
        contact => format!("{tool} ({contact})"),
    }
}

/// Whether text will do as a contact: something, on one line, that
/// cannot be mistaken for the end of the parentheses it goes in.
pub fn contact_is_usable(contact: &str) -> bool {
    let contact = contact.trim();
    !contact.is_empty()
        && contact.len() <= 200
        && !contact
            .chars()
            .any(|c| c.is_control() || c == '(' || c == ')')
}

/// The most revisions the API returns in one response when their text is
/// asked for.
pub const BATCH: u32 = 50;

/// An article on a particular wiki.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Article {
    /// The wiki's host name, such as `en.wikipedia.org`.
    pub host: String,
    pub title: String,
}

impl Article {
    /// Reads what someone might type or paste: an article's address
    /// (`https://de.wikipedia.org/wiki/Schokolade`), or just its title,
    /// which is taken to be on the English Wikipedia.
    pub fn parse(input: &str) -> Option<Article> {
        let input = input.trim();
        let address = input
            .strip_prefix("https://")
            .or_else(|| input.strip_prefix("http://"));
        let (host, title) = match address {
            Some(address) => {
                let (host, path) = address.split_once('/')?;
                let title = path.strip_prefix("wiki/")?;
                // Drop any ?query or #section, and undo the address's
                // encoding of the title.
                let title = title.split(['?', '#']).next()?;
                (host.to_string(), percent_decode(title).replace('_', " "))
            }
            None => ("en.wikipedia.org".to_string(), input.to_string()),
        };
        let host_ok = !host.is_empty()
            && host
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
        (host_ok && !title.is_empty()).then_some(Article { host, title })
    }

    /// The address to fetch for the next batch of up to `limit`
    /// revisions. `next` is [`Batch::next`] from the previous response,
    /// or empty for the first request. `origin=*` asks the server to
    /// allow a page on any site to read the response.
    pub fn request(&self, next: &str, limit: u32) -> String {
        format!(
            "https://{}/w/api.php?action=query&prop=revisions&titles={}&redirects=1\
             &rvprop=ids%7Ctimestamp%7Cuser%7Ccomment%7Ccontent&rvslots=main\
             &rvlimit={}&rvdir=newer&format=json&formatversion=2&origin=*{next}",
            self.host,
            percent_encode(&self.title),
            limit.clamp(1, BATCH),
        )
    }
}

/// What one response held.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Batch {
    /// The article's title as the wiki gives it (after any redirect).
    pub title: String,
    /// The revisions, oldest first. A revision whose text has been
    /// hidden by the wiki's administrators is left out.
    pub versions: Vec<SourceVersion>,
    /// What to pass to [`Article::request`] for the batch after this
    /// one, or `None` if this was the last.
    pub next: Option<String>,
}

#[derive(Deserialize)]
struct Response {
    error: Option<ApiError>,
    #[serde(rename = "continue")]
    next: Option<serde_json::Map<String, serde_json::Value>>,
    query: Option<Query>,
}

#[derive(Deserialize)]
struct ApiError {
    info: String,
}

#[derive(Deserialize)]
struct Query {
    pages: Vec<Page>,
}

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    title: String,
    #[serde(default)]
    missing: bool,
    #[serde(default)]
    invalid: bool,
    #[serde(default)]
    revisions: Vec<Revision>,
}

#[derive(Deserialize)]
struct Revision {
    revid: u64,
    user: Option<String>,
    /// An editor with no account, known by IP address.
    #[serde(default)]
    anon: bool,
    /// An editor with no account, known by a temporary name.
    #[serde(default)]
    temp: bool,
    timestamp: String,
    comment: Option<String>,
    slots: Option<Slots>,
}

#[derive(Deserialize)]
struct Slots {
    main: Slot,
}

#[derive(Deserialize)]
struct Slot {
    content: Option<String>,
}

/// Reads one response of the API.
pub fn parse(response: &str) -> Result<Batch, String> {
    let response: Response =
        serde_json::from_str(response).map_err(|e| format!("unexpected response: {e}"))?;
    if let Some(error) = response.error {
        return Err(error.info);
    }
    let page = response
        .query
        .and_then(|query| query.pages.into_iter().next())
        .ok_or("the response names no page")?;
    if page.missing || page.invalid {
        return Err(format!("there is no article called “{}”", page.title));
    }

    let mut versions = Vec::with_capacity(page.revisions.len());
    for revision in page.revisions {
        let Some(text) = revision.slots.and_then(|slots| slots.main.content) else {
            continue;
        };
        versions.push(SourceVersion {
            id: revision.revid.to_string(),
            anonymous: revision.anon || revision.temp || revision.user.is_none(),
            author: revision.user.unwrap_or_else(|| "(hidden)".into()),
            timestamp: timestamp(&revision.timestamp)
                .ok_or_else(|| format!("unexpected timestamp {:?}", revision.timestamp))?,
            comment: revision.comment.unwrap_or_default(),
            text,
        });
    }
    // Every field of "continue" goes back as a parameter of the next
    // request.
    let next = response.next.map(|fields| {
        fields
            .iter()
            .map(|(key, value)| {
                let value = value
                    .as_str()
                    .map_or_else(|| value.to_string(), str::to_string);
                format!("&{}={}", percent_encode(key), percent_encode(&value))
            })
            .collect()
    });
    Ok(Batch {
        title: page.title,
        versions,
        next,
    })
}

/// Seconds since the Unix epoch of a `2003-08-20T13:41:20Z` timestamp.
fn timestamp(text: &str) -> Option<i64> {
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let bytes = text.as_bytes();
    let punctuation = [
        (4, b'-'),
        (7, b'-'),
        (10, b'T'),
        (13, b':'),
        (16, b':'),
        (19, b'Z'),
    ];
    if text.len() != 20 || punctuation.iter().any(|&(at, c)| bytes[at] != c) {
        return None;
    }
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    // Days from the epoch to a civil date (Howard Hinnant's algorithm):
    // count years from March so the leap day falls at the year's end.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year.rem_euclid(400);
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(((days * 24 + hour) * 60 + minute) * 60 + second)
}

/// Encodes text as one value in a query string.
fn percent_encode(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
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
                decoded.push(bytes[i]);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn article(host: &str, title: &str) -> Option<Article> {
        Some(Article {
            host: host.into(),
            title: title.into(),
        })
    }

    #[test]
    fn a_title_alone_is_on_the_english_wikipedia() {
        assert_eq!(
            Article::parse("  Chocolate "),
            article("en.wikipedia.org", "Chocolate")
        );
        assert_eq!(
            Article::parse("Capital punishment"),
            article("en.wikipedia.org", "Capital punishment")
        );
    }

    #[test]
    fn an_address_gives_the_wiki_and_the_decoded_title() {
        assert_eq!(
            Article::parse("https://de.wikipedia.org/wiki/Gr%C3%BCner_Tee?action=history#Sorten"),
            article("de.wikipedia.org", "Grüner Tee")
        );
        assert_eq!(
            Article::parse("http://en.wikipedia.org/wiki/AC/DC"),
            article("en.wikipedia.org", "AC/DC")
        );
    }

    #[test]
    fn the_tool_names_itself_and_whoever_runs_it() {
        let tool = format!("historyflow/{}", crate::version());
        assert_eq!(user_agent(""), tool);
        assert_eq!(
            user_agent(" ada@example.com "),
            format!("{tool} (ada@example.com)")
        );
        assert!(contact_is_usable("https://example.com/ada"));
        for bad in ["", "  ", "a\nb", "a) evil/1.0 (b"] {
            assert!(!contact_is_usable(bad), "{bad:?}");
        }
    }

    #[test]
    fn nonsense_is_not_an_article() {
        assert_eq!(Article::parse(""), None);
        assert_eq!(Article::parse("https://en.wikipedia.org/"), None);
        assert_eq!(
            Article::parse("https://en.wikipedia.org/w/index.php?title=X"),
            None
        );
        assert_eq!(Article::parse("https://evil host/wiki/X"), None);
    }

    #[test]
    fn the_request_encodes_the_title_and_carries_the_continuation() {
        let article = Article::parse("AC/DC & friends").unwrap();
        assert_eq!(
            article.request("&rvcontinue=20030820134120%7C1318623&continue=%7C%7C", 20),
            "https://en.wikipedia.org/w/api.php?action=query&prop=revisions\
             &titles=AC%2FDC%20%26%20friends&redirects=1\
             &rvprop=ids%7Ctimestamp%7Cuser%7Ccomment%7Ccontent&rvslots=main\
             &rvlimit=20&rvdir=newer&format=json&formatversion=2&origin=*\
             &rvcontinue=20030820134120%7C1318623&continue=%7C%7C"
        );
        assert!(article.request("", 9999).contains("&rvlimit=50&"));
    }

    const RESPONSE: &str = r#"{
      "continue": {"rvcontinue": "20030820134120|1318623", "continue": "||"},
      "query": {"pages": [{"pageid": 7089, "ns": 0, "title": "Chocolate", "revisions": [
        {"revid": 236681, "parentid": 0, "user": "Dmerrill", "timestamp": "2001-10-12T09:01:17Z",
         "comment": "first", "slots": {"main": {"contentmodel": "wikitext", "content": "Chocolate is good."}}},
        {"revid": 236682, "parentid": 236681, "user": "203.0.113.7", "anon": true,
         "timestamp": "2001-10-12T09:05:00Z", "comment": "",
         "slots": {"main": {"content": "Chocolate is very good."}}},
        {"revid": 236683, "user": "Vandal", "timestamp": "2001-10-13T00:00:00Z",
         "slots": {"main": {"texthidden": true}}},
        {"revid": 236684, "userhidden": true, "commenthidden": true, "timestamp": "2001-10-14T00:00:00Z",
         "slots": {"main": {"content": "Chocolate."}}},
        {"revid": 236685, "user": "~2025-12345", "temp": true, "timestamp": "2025-11-01T00:00:00Z",
         "comment": "c", "slots": {"main": {"content": "Chocolate!"}}}
      ]}]}
    }"#;

    #[test]
    fn a_response_becomes_versions_and_a_continuation() {
        let batch = parse(RESPONSE).unwrap();
        assert_eq!(batch.title, "Chocolate");
        assert_eq!(
            batch.next.as_deref(),
            Some("&continue=%7C%7C&rvcontinue=20030820134120%7C1318623")
        );
        let seen: Vec<_> = batch
            .versions
            .iter()
            .map(|v| {
                (
                    v.id.as_str(),
                    v.author.as_str(),
                    v.anonymous,
                    v.timestamp,
                    v.text.as_str(),
                )
            })
            .collect();
        assert_eq!(
            seen,
            [
                (
                    "236681",
                    "Dmerrill",
                    false,
                    1_002_877_277,
                    "Chocolate is good."
                ),
                (
                    "236682",
                    "203.0.113.7",
                    true,
                    1_002_877_500,
                    "Chocolate is very good."
                ),
                // 236683's text is hidden, so it is not a version.
                ("236684", "(hidden)", true, 1_003_017_600, "Chocolate."),
                ("236685", "~2025-12345", true, 1_761_955_200, "Chocolate!"),
            ]
        );
        assert_eq!(batch.versions[0].comment, "first");
    }

    #[test]
    fn the_last_response_has_no_continuation() {
        let last =
            r#"{"batchcomplete": true, "query": {"pages": [{"title": "X", "revisions": []}]}}"#;
        assert_eq!(parse(last).unwrap().next, None);
    }

    #[test]
    fn failures_are_reported_in_words() {
        let missing = r#"{"query": {"pages": [{"title": "Nonesuch", "missing": true}]}}"#;
        assert_eq!(
            parse(missing).unwrap_err(),
            "there is no article called “Nonesuch”"
        );
        let error =
            r#"{"error": {"code": "ratelimited", "info": "You've exceeded your rate limit."}}"#;
        assert_eq!(
            parse(error).unwrap_err(),
            "You've exceeded your rate limit."
        );
        assert!(
            parse("<html>")
                .unwrap_err()
                .starts_with("unexpected response")
        );
    }

    #[test]
    fn timestamps_convert_across_leap_years() {
        assert_eq!(timestamp("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(timestamp("2000-02-29T12:00:00Z"), Some(951_825_600));
        assert_eq!(timestamp("2024-03-01T00:00:00Z"), Some(1_709_251_200));
        assert_eq!(timestamp("2024-03-01 00:00:00"), None);
    }
}
