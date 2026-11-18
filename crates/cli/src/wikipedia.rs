//! Fetches a Wikipedia article's history into an archive, so that it can
//! be opened later with nothing asked of the wiki.
//!
//! What to ask and how to read the answers is `core::wikipedia`, the
//! same as the page uses; this adds the asking, by running `curl`.

use std::process::Command;

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

/// Fetches an address with `curl`, naming this tool and its version as
/// Wikimedia asks of tools. When the wiki says to slow down, waits as
/// long as it asks (saying so through `waiting`, in seconds) and tries
/// again, a few times.
pub fn curl(url: &str, mut waiting: impl FnMut(u64)) -> Result<String, Error> {
    let agent = concat!("historyflow/", env!("CARGO_PKG_VERSION"));
    for attempt in 0.. {
        let output = Command::new("curl")
            .args(["--silent", "--show-error", "--include", "--compressed"])
            .args(["--max-time", "120", "--user-agent", agent, "--", url])
            .output()
            .map_err(|e| format!("cannot run curl, which fetches from the wiki: {e}"))?;
        if !output.status.success() {
            let said = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Wikipedia: {}", said.trim()).into());
        }
        let answer =
            String::from_utf8(output.stdout).map_err(|_| "Wikipedia: the answer was not text")?;
        let answer = Answer::read(&answer).ok_or("Wikipedia: the answer was not HTTP")?;
        match answer.status {
            200 => return Ok(answer.body.to_string()),
            429 if attempt < 4 => {
                let seconds = answer.retry_after.unwrap_or(5 * (attempt + 1)).min(300);
                waiting(seconds);
                std::thread::sleep(std::time::Duration::from_secs(seconds));
            }
            status => return Err(format!("Wikipedia: HTTP {status}").into()),
        }
    }
    unreachable!("the loop ends by returning")
}

/// What `curl --include` printed: the status, the one header of
/// interest, and the body.
#[derive(Debug, PartialEq, Eq)]
struct Answer<'a> {
    status: u16,
    /// How long the server asks to be left alone, in seconds.
    retry_after: Option<u64>,
    body: &'a str,
}

impl Answer<'_> {
    fn read(answer: &str) -> Option<Answer<'_>> {
        let (head, body) = answer
            .split_once("\r\n\r\n")
            .or_else(|| answer.split_once("\n\n"))?;
        let mut lines = head.lines();
        // "HTTP/2 200", "HTTP/1.1 429 Too Many Requests".
        let status = lines.next()?.split_whitespace().nth(1)?.parse().ok()?;
        let retry_after = lines
            .filter_map(|line| line.split_once(':'))
            .find(|(name, _)| name.eq_ignore_ascii_case("retry-after"))
            .and_then(|(_, value)| value.trim().parse().ok());
        Some(Answer {
            status,
            retry_after,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Answer;

    #[test]
    fn an_answer_is_its_status_its_wait_and_its_body() {
        assert_eq!(
            Answer::read("HTTP/2 200 \r\ncontent-type: application/json\r\n\r\n{\"a\":\n\n1}"),
            Some(Answer {
                status: 200,
                retry_after: None,
                body: "{\"a\":\n\n1}"
            })
        );
        assert_eq!(
            Answer::read("HTTP/1.1 429 Too Many Requests\r\nRetry-After: 47\r\n\r\nslow down"),
            Some(Answer {
                status: 429,
                retry_after: Some(47),
                body: "slow down"
            })
        );
        assert_eq!(Answer::read("nonsense"), None);
    }
}
