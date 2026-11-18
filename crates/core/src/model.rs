//! The common representation every data source is turned into.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Position of a version in its history, oldest first.
pub type VersionIndex = u32;

/// Identity of a piece of text across versions. Two atoms in different
/// versions with the same token are the same text, introduced once, by
/// the token's origin version. A token occurs at most once in a version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenId(pub u32);

/// Index into [`History::authors`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AuthorId(pub u32);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Author {
    pub name: String,
    /// True for contributors with no account (a Wikipedia IP address).
    /// The original drew these in white and greys.
    pub anonymous: bool,
}

/// One version of a document as a data source supplies it. Sources yield
/// these oldest first.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceVersion {
    /// The source's own name for the version: a commit hash, a revision
    /// number.
    pub id: String,
    pub author: String,
    pub anonymous: bool,
    /// Seconds since the Unix epoch.
    pub timestamp: i64,
    /// The commit message or edit summary.
    pub comment: String,
    pub text: String,
}

/// `len` tokens with consecutive ids starting at `first`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Run {
    pub first: TokenId,
    pub len: u32,
}

/// One version: its metadata and its content as a sequence of tokens.
///
/// The sequence is stored as maximal runs of consecutive ids. Text that
/// arrives together gets consecutive ids and mostly stays together, so a
/// version is a few hundred runs where it would be tens of thousands of
/// ids.
#[derive(Debug, Clone)]
pub struct Version {
    pub id: String,
    pub author: AuthorId,
    pub timestamp: i64,
    pub comment: String,
    runs: Vec<Run>,
    token_count: u32,
    weight: u64,
}

impl Version {
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }

    /// The version's tokens in document order.
    pub fn tokens(&self) -> impl Iterator<Item = TokenId> + '_ {
        self.runs
            .iter()
            .flat_map(|run| (run.first.0..run.first.0 + run.len).map(TokenId))
    }

    pub fn token_count(&self) -> u32 {
        self.token_count
    }

    /// The sum of the tokens' weights: the length of this version's line
    /// in the plot.
    pub fn weight(&self) -> u64 {
        self.weight
    }
}

/// A passage shared by two versions: `len` tokens starting at token
/// position `old` in the older version and `new` in the newer one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Link {
    pub old: u32,
    pub new: u32,
    pub len: u32,
}

/// A document's versions with every token attributed. Built by a
/// [`HistoryBuilder`](crate::HistoryBuilder).
#[derive(Debug, Default)]
pub struct History {
    authors: Vec<Author>,
    author_ids: HashMap<Author, AuthorId>,
    versions: Vec<Version>,
    /// Per token: the version that introduced it.
    origin: Vec<VersionIndex>,
    /// Per token: its size in the plot.
    weight: Vec<u32>,
    /// Per token: index into `texts`.
    text: Vec<u32>,
    /// Each distinct atom text, once.
    texts: Vec<Box<str>>,
    text_ids: HashMap<Box<str>, u32>,
}

impl History {
    pub fn authors(&self) -> &[Author] {
        &self.authors
    }

    pub fn versions(&self) -> &[Version] {
        &self.versions
    }

    /// The number of tokens ever created.
    pub fn token_count(&self) -> usize {
        self.origin.len()
    }

    /// The version that introduced the token.
    pub fn origin(&self, token: TokenId) -> VersionIndex {
        self.origin[token.0 as usize]
    }

    /// The author of the version that introduced the token.
    pub fn author(&self, token: TokenId) -> AuthorId {
        self.versions[self.origin(token) as usize].author
    }

    /// The token's size in the plot.
    pub fn weight(&self, token: TokenId) -> u32 {
        self.weight[token.0 as usize]
    }

    /// The token's text as it was compared: the atom, without surrounding
    /// whitespace.
    pub fn text(&self, token: TokenId) -> &str {
        &self.texts[self.text[token.0 as usize] as usize]
    }

    /// The passages `older` and `newer` share, as maximal runs that are
    /// contiguous in both, ordered by position in `newer`. These are the
    /// ribbons between two revision lines. Tokens in neither's links were
    /// deleted (if in `older`) or inserted or restored (if in `newer`);
    /// links whose `old` positions are out of order are moves.
    pub fn links(&self, older: VersionIndex, newer: VersionIndex) -> Vec<Link> {
        let older = &self.versions[older as usize];
        let newer = &self.versions[newer as usize];
        // The older version's runs as disjoint id intervals, sorted by id,
        // each with its token position.
        let mut intervals = Vec::with_capacity(older.runs.len());
        let mut position = 0;
        for run in &older.runs {
            intervals.push((run.first.0, run.len, position));
            position += run.len;
        }
        intervals.sort_unstable();

        let mut links: Vec<Link> = Vec::new();
        let mut position = 0;
        for run in &newer.runs {
            let (first, end) = (run.first.0, run.first.0 + run.len);
            let from = intervals.partition_point(|&(start, len, _)| start + len <= first);
            for &(start, len, old_position) in &intervals[from..] {
                if start >= end {
                    break;
                }
                let lo = first.max(start);
                let hi = end.min(start + len);
                let link = Link {
                    old: old_position + (lo - start),
                    new: position + (lo - first),
                    len: hi - lo,
                };
                // Tokens adjacent in both versions need not have
                // consecutive ids, so a passage can span several runs:
                // join a link that continues the one before it.
                match links.last_mut() {
                    Some(last)
                        if last.old + last.len == link.old && last.new + last.len == link.new =>
                    {
                        last.len += link.len;
                    }
                    _ => links.push(link),
                }
            }
            position += run.len;
        }
        links
    }

    pub(crate) fn intern_text(&mut self, text: &str) -> u32 {
        if let Some(&id) = self.text_ids.get(text) {
            return id;
        }
        let id = self.texts.len() as u32;
        self.texts.push(text.into());
        self.text_ids.insert(text.into(), id);
        id
    }

    pub(crate) fn text_count(&self) -> u32 {
        self.texts.len() as u32
    }

    pub(crate) fn text_id(&self, token: TokenId) -> u32 {
        self.text[token.0 as usize]
    }

    pub(crate) fn new_token(&mut self, origin: VersionIndex, weight: u32, text: u32) -> TokenId {
        let id = TokenId(self.origin.len() as u32);
        self.origin.push(origin);
        self.weight.push(weight);
        self.text.push(text);
        id
    }

    pub(crate) fn push_version(&mut self, source: &SourceVersion, tokens: &[TokenId]) {
        let author = Author {
            name: source.author.clone(),
            anonymous: source.anonymous,
        };
        let author = match self.author_ids.get(&author) {
            Some(&id) => id,
            None => {
                let id = AuthorId(self.authors.len() as u32);
                self.authors.push(author.clone());
                self.author_ids.insert(author, id);
                id
            }
        };
        let mut runs: Vec<Run> = Vec::new();
        for &token in tokens {
            match runs.last_mut() {
                Some(run) if run.first.0 + run.len == token.0 => run.len += 1,
                _ => runs.push(Run {
                    first: token,
                    len: 1,
                }),
            }
        }
        self.versions.push(Version {
            id: source.id.clone(),
            author,
            timestamp: source.timestamp,
            comment: source.comment.clone(),
            runs,
            token_count: tokens.len() as u32,
            weight: tokens.iter().map(|&t| u64::from(self.weight(t))).sum(),
        });
    }
}
