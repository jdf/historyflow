//! Fetches a Wikipedia article's history into an archive, so that it can
//! be opened later with nothing asked of the wiki.
//!
//! What to ask and how to read the answers is `core::wikipedia`, the
//! same as the page uses; this adds the asking.

use std::process::Command;
use std::time::{Duration, Instant};

use historyflow_core::wikipedia::{self, Article};
use historyflow_core::{Archive, Mode};

use crate::Error;

/// Reads the first `revisions` revisions of `article` (its title or
/// address), asking for each batch's address with `get` and reporting
/// the number of revisions read so far to `progress` after each.
pub fn read(
    article: &str,
    revisions: u32,
    mut get: impl FnMut(&str) -> Result<String, Error>,
    mut progress: impl FnMut(usize),
) -> Result<Archive, Error> {
    let article = Article::parse(article).ok_or_else(|| {
        format!(
            "{article} is not an article's title, or its address on a Wikipedia \
             (https://en.wikipedia.org/wiki/Chocolate)"
        )
    })?;
    let wanted = revisions as usize;
    let mut archive = Archive::new(article.title.clone(), Mode::Prose);
    let mut next = String::new();
    while archive.len() < wanted {
        let remaining = (wanted - archive.len()) as u32;
        let batch = wikipedia::parse(&get(&article.request(&next, remaining))?)?;
        archive.title = batch.title;
        // A batch can hold more than was asked for only if the wiki
        // ignores the limit; keep to it regardless.
        for version in batch.versions.iter().take(wanted - archive.len()) {
            archive.push(version);
        }
        progress(archive.len());
        match batch.next {
            Some(following) => next = following,
            None => break,
        }
    }
    Ok(archive)
}

/// How the wiki can reach whoever is running this, which Wikimedia asks
/// every tool to say. Like git's user.email, it is each person's to
/// set, and nothing is fetched until it is. Looked for in `given` (the
/// command line), then the environment's `HISTORYFLOW_CONTACT`, then
/// git's configuration, as `historyflow.contact`.
pub fn contact(given: Option<&str>) -> Result<String, Error> {
    let from_environment = std::env::var("HISTORYFLOW_CONTACT").ok();
    let found = given
        .map(str::to_string)
        .or(from_environment)
        .or_else(from_git);
    let Some(contact) = found.filter(|contact| !contact.trim().is_empty()) else {
        return Err(
            "Wikipedia asks tools to say how to reach whoever is running them, \
             so tell Historyflow, once, how to reach you: an email address, \
             a website, or your user name on a wiki.\n\n    \
             git config --global historyflow.contact you@example.com\n\n\
             It is sent to the wiki with each request, and nowhere else. \
             Or give it for one run with --contact, or in HISTORYFLOW_CONTACT."
                .into(),
        );
    };
    if !wikipedia::contact_is_usable(&contact) {
        return Err(
            format!("{contact:?} will not do as a contact: one line, without parentheses").into(),
        );
    }
    Ok(contact.trim().to_string())
}

fn from_git() -> Option<String> {
    let output = Command::new("git")
        .args(["config", "--get", "historyflow.contact"])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Makes a way of fetching considerate: Wikimedia asks that a request it
/// took over a second to answer be followed by five seconds' rest.
pub fn rested(
    mut get: impl FnMut(&str) -> Result<String, Error>,
) -> impl FnMut(&str) -> Result<String, Error> {
    let mut slow = false;
    move |url| {
        if slow {
            std::thread::sleep(Duration::from_secs(5));
        }
        let asked = Instant::now();
        let answer = get(url);
        slow = asked.elapsed() > Duration::from_secs(1);
        answer
    }
}

/// Asks the wiki for things over HTTPS, naming this tool, its version
/// and its user's contact as Wikimedia asks of tools.
pub struct Client {
    agent: ureq::Agent,
}

impl Client {
    pub fn new(contact: &str) -> Client {
        let config = ureq::Agent::config_builder()
            .user_agent(wikipedia::user_agent(contact))
            .https_only(true)
            .timeout_global(Some(Duration::from_secs(120)))
            // A status is read here, where a 429 means wait.
            .http_status_as_error(false)
            .build();
        Client {
            agent: config.new_agent(),
        }
    }

    /// Fetches an address. When the wiki says to slow down, waits as
    /// long as it asks (saying so through `waiting`, in seconds) and
    /// tries again, a few times.
    pub fn get(&self, url: &str, mut waiting: impl FnMut(u64)) -> Result<String, Error> {
        for attempt in 0.. {
            let mut answer = self
                .agent
                .get(url)
                .call()
                .map_err(|e| format!("Wikipedia: {e}"))?;
            match answer.status().as_u16() {
                // Fifty revisions of a long article, each in full, are
                // many times the library's own limit.
                200 => {
                    return Ok(answer
                        .body_mut()
                        .with_config()
                        .limit(1 << 30)
                        .read_to_string()
                        .map_err(|e| format!("Wikipedia: {e}"))?);
                }
                429 if attempt < 4 => {
                    let asked = answer
                        .headers()
                        .get("retry-after")
                        .and_then(|value| value.to_str().ok())
                        .and_then(|value| value.trim().parse().ok());
                    let seconds = asked.unwrap_or(5 * (attempt + 1)).min(300);
                    waiting(seconds);
                    std::thread::sleep(Duration::from_secs(seconds));
                }
                status => return Err(format!("Wikipedia: HTTP {status}").into()),
            }
        }
        unreachable!("the loop ends by returning")
    }
}
