//! Giving every atom of every version a token identity.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

use imara_diff::{Algorithm, Diff, Token};

use crate::model::{History, SourceVersion, TokenId, VersionIndex};
use crate::segment::{Lines, Prose, Segmenter, Sentences};

/// How an atom in a new version is judged to be the same text as one in
/// an earlier version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Matching {
    /// WikiWho-style, for prose. Whole blocks, then whole units, are
    /// matched by content against the previous version and then against
    /// every version before it, wherever they sit in the document; what
    /// is left is compared atom by atom with the previous version.
    ///
    /// So a moved paragraph keeps its identity, and text that was deleted
    /// and later restored comes back as the tokens it was, still
    /// attributed to whoever first wrote it.
    Historical,
    /// A strict diff of each version against the one before it, and
    /// nothing else: the semantics of `git blame`. Order is preserved, so
    /// nothing is ever a move, and restored text is new text by whoever
    /// restored it.
    ///
    /// This is the right choice for source code, where lines such as `}`
    /// recur so often that matching them by content across the history
    /// would be meaningless.
    Pairwise,
}

/// What a token's size in the plot measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    /// The atom's length in characters, as in the original.
    Chars,
    /// One per atom: with [`Lines`], a version's length is its line count.
    Atoms,
}

/// How a history is built from its versions' text.
pub struct Config {
    pub segmenter: Box<dyn Segmenter>,
    pub matching: Matching,
    pub measure: Measure,
}

impl Config {
    /// Prose and wiki markup: word tokens, matched across the whole
    /// history.
    pub fn prose() -> Config {
        Config {
            segmenter: Box::new(Prose),
            matching: Matching::Historical,
            measure: Measure::Chars,
        }
    }

    /// The original's granularity: whole sentences, split at periods and
    /// HTML tags. Matched across the whole history, since the original's
    /// figures show restored text keeping its authors' colours.
    pub fn sentences_2004() -> Config {
        Config {
            segmenter: Box::new(Sentences),
            matching: Matching::Historical,
            measure: Measure::Chars,
        }
    }

    /// Source code: whole lines, each version diffed strictly against
    /// the one before it.
    pub fn source_lines() -> Config {
        Config {
            segmenter: Box::new(Lines),
            matching: Matching::Pairwise,
            measure: Measure::Atoms,
        }
    }
}

/// Every distinct block or unit seen so far, as the tokens it was made
/// of, findable by a hash of its content.
#[derive(Default)]
struct Passages {
    by_hash: HashMap<u64, Vec<u32>>,
    tokens: Vec<Box<[TokenId]>>,
}

/// Builds a [`History`] one version at a time, oldest first. The history
/// is usable between pushes, so a slow source can be shown as it loads.
pub struct HistoryBuilder {
    config: Config,
    history: History,
    /// The previous version's tokens.
    previous: Vec<TokenId>,
    /// Per token: one more than the index of the last version it was in,
    /// or 0. Tells whether a token is already placed in the version being
    /// built, and whether it was in the previous one.
    last_seen: Vec<u32>,
    /// Per token: its position in the last version it was in.
    position: Vec<u32>,
    blocks: Passages,
    units: Passages,
    diff: Diff,
}

const UNASSIGNED: TokenId = TokenId(u32::MAX);

impl HistoryBuilder {
    pub fn new(config: Config) -> HistoryBuilder {
        HistoryBuilder {
            config,
            history: History::default(),
            previous: Vec::new(),
            last_seen: Vec::new(),
            position: Vec::new(),
            blocks: Passages::default(),
            units: Passages::default(),
            diff: Diff::default(),
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn history(&self) -> &History {
        &self.history
    }

    pub fn finish(self) -> History {
        self.history
    }

    /// Adds the next version and returns its index. The version's tokens
    /// correspond one to one, in order, with the atoms the configured
    /// segmenter yields for `source.text`.
    pub fn push(&mut self, source: &SourceVersion) -> VersionIndex {
        let text = source.text.as_str();
        let seg = self.config.segmenter.segment(text);
        let index = self.history.versions().len() as VersionIndex;
        let stamp = index + 1;

        let texts: Vec<u32> = seg
            .atoms
            .iter()
            .map(|atom| self.history.intern_text(&text[atom.clone()]))
            .collect();
        let mut tokens = vec![UNASSIGNED; texts.len()];

        // Blocks and units no passage matched, to be recorded once their
        // atoms have tokens. Atom ranges.
        let mut new_blocks = Vec::new();
        let mut new_units = Vec::new();

        if self.config.matching == Matching::Historical {
            // Every block gets its chance before any unit does, so a unit
            // of an edited block cannot take the tokens of an identical
            // unit inside a block that is itself unchanged.
            let units: Vec<_> = seg.units().collect();
            let mut unmatched_units = Vec::new();
            for block in seg.blocks() {
                let atoms = units[block.start].start..units[block.end - 1].end;
                let (texts, out) = (&texts[atoms.clone()], &mut tokens[atoms.clone()]);
                if !self.claim(Level::Block, texts, stamp, out) {
                    unmatched_units.extend_from_slice(&units[block]);
                    new_blocks.push(atoms);
                }
            }
            for unit in unmatched_units {
                let (texts, out) = (&texts[unit.clone()], &mut tokens[unit.clone()]);
                if !self.claim(Level::Unit, texts, stamp, out) {
                    new_units.push(unit);
                }
            }
        }

        // Compare what is still unassigned with what is left of the
        // previous version, in order.
        let old: Vec<TokenId> = self
            .previous
            .iter()
            .copied()
            .filter(|t| self.last_seen[t.0 as usize] != stamp)
            .collect();
        let new: Vec<usize> = (0..tokens.len())
            .filter(|&i| tokens[i] == UNASSIGNED)
            .collect();
        let old_texts: Vec<Token> = old
            .iter()
            .map(|&t| Token(self.history.text_id(t)))
            .collect();
        let new_texts: Vec<Token> = new.iter().map(|&i| Token(texts[i])).collect();
        self.diff.compute_with(
            Algorithm::Histogram,
            &old_texts,
            &new_texts,
            self.history.text_count(),
        );
        // The hunks are the differing stretches; everything between them,
        // and after the last, is common to both and pairs off in order.
        let end = (old.len() as u32, new.len() as u32);
        let hunks = self.diff.hunks().map(|h| (h.before, h.after));
        let (mut i, mut j) = (0, 0);
        for (before, after) in hunks.chain([(end.0..end.0, end.1..end.1)]) {
            while i < before.start as usize {
                tokens[new[j]] = old[i];
                i += 1;
                j += 1;
            }
            (i, j) = (before.end as usize, after.end as usize);
        }

        // Whatever matched nothing is new text, introduced here.
        for (i, token) in tokens.iter_mut().enumerate() {
            if *token == UNASSIGNED {
                let weight = match self.config.measure {
                    Measure::Chars => text[seg.atoms[i].clone()].chars().count() as u32,
                    Measure::Atoms => 1,
                };
                *token = self.history.new_token(index, weight, texts[i]);
                self.last_seen.push(0);
                self.position.push(0);
            }
            self.last_seen[token.0 as usize] = stamp;
            self.position[token.0 as usize] = i as u32;
        }

        for atoms in new_blocks {
            self.blocks.record(&texts[atoms.clone()], &tokens[atoms]);
        }
        for atoms in new_units {
            self.units.record(&texts[atoms.clone()], &tokens[atoms]);
        }

        self.history.push_version(source, &tokens);
        self.previous = tokens;
        index
    }

    /// Looks for a recorded passage with exactly this content whose
    /// tokens are all still free in the version being built. One that was
    /// in the previous version is preferred, the earliest in that version
    /// first, so that identical passages keep their order; failing that,
    /// the oldest recorded. On success writes its tokens to `out` and
    /// marks them placed.
    fn claim(&mut self, level: Level, texts: &[u32], stamp: u32, out: &mut [TokenId]) -> bool {
        let passages = match level {
            Level::Block => &self.blocks,
            Level::Unit => &self.units,
        };
        let Some(candidates) = passages.by_hash.get(&hash(texts)) else {
            return false;
        };
        let usable = |candidate: u32, in_previous: bool| {
            let passage = &passages.tokens[candidate as usize];
            passage.len() == texts.len()
                && passage.iter().zip(texts).all(|(&t, &text)| {
                    let seen = self.last_seen[t.0 as usize];
                    self.history.text_id(t) == text
                        && seen != stamp
                        && (!in_previous || seen + 1 == stamp)
                })
        };
        let found = candidates
            .iter()
            .copied()
            .filter(|&c| usable(c, true))
            .min_by_key(|&c| self.position[passages.tokens[c as usize][0].0 as usize])
            .or_else(|| candidates.iter().copied().find(|&c| usable(c, false)));
        let Some(found) = found else {
            return false;
        };
        out.copy_from_slice(&passages.tokens[found as usize]);
        for token in out {
            self.last_seen[token.0 as usize] = stamp;
        }
        true
    }
}

#[derive(Clone, Copy)]
enum Level {
    Block,
    Unit,
}

impl Passages {
    fn record(&mut self, texts: &[u32], tokens: &[TokenId]) {
        let ids = self.by_hash.entry(hash(texts)).or_default();
        if ids.iter().any(|&id| *self.tokens[id as usize] == *tokens) {
            return;
        }
        ids.push(self.tokens.len() as u32);
        self.tokens.push(tokens.into());
    }
}

fn hash(texts: &[u32]) -> u64 {
    let mut hasher = DefaultHasher::new();
    texts.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Link;

    /// Builds a history whose versions are written by authors "0", "1", …
    /// in turn.
    fn build(config: Config, texts: &[&str]) -> History {
        let mut builder = HistoryBuilder::new(config);
        for (i, text) in texts.iter().enumerate() {
            builder.push(&SourceVersion {
                id: format!("v{i}"),
                author: i.to_string(),
                text: text.to_string(),
                ..SourceVersion::default()
            });
        }
        builder.finish()
    }

    /// Each token of a version as `text@origin`.
    fn blame(history: &History, version: usize) -> String {
        history.versions()[version]
            .tokens()
            .map(|t| format!("{}@{}", history.text(t), history.origin(t)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn link(old: u32, new: u32, len: u32) -> Link {
        Link { old, new, len }
    }

    #[test]
    fn unchanged_text_keeps_its_origin_and_edits_get_the_editor() {
        let h = build(
            Config::prose(),
            &["the quick fox", "the quick brown fox", "the brown fox"],
        );
        assert_eq!(blame(&h, 1), "the@0 quick@0 brown@1 fox@0");
        assert_eq!(blame(&h, 2), "the@0 brown@1 fox@0");
        assert_eq!(h.links(0, 1), [link(0, 0, 2), link(2, 3, 1)]);
        assert_eq!(h.links(1, 2), [link(0, 0, 1), link(2, 1, 2)]);
        assert_eq!(h.versions()[1].weight(), 16);
    }

    #[test]
    fn historical_restores_deleted_text_to_its_original_authors() {
        let page = "Alpha beta. Gamma delta.\n\nEpsilon zeta.";
        let h = build(Config::prose(), &["Alpha beta.", page, "VANDALISED", page]);
        assert_eq!(blame(&h, 2), "VANDALISED@2");
        assert_eq!(blame(&h, 3), blame(&h, 1));
        // Nothing connects the vandalised version to its neighbours:
        // the black gutter.
        assert_eq!(h.links(1, 2), []);
        assert_eq!(h.links(2, 3), []);
        assert_eq!(h.links(1, 3), [link(0, 0, 9)]);
    }

    #[test]
    fn pairwise_attributes_restored_text_to_whoever_restored_it() {
        let h = build(Config::source_lines(), &["a\nb\n", "", "a\nb\n"]);
        assert_eq!(blame(&h, 2), "a@2 b@2");
    }

    #[test]
    fn historical_follows_a_moved_paragraph() {
        let h = build(
            Config::prose(),
            &[
                "One one.\n\nTwo two.\n\nThree three.",
                "Three three.\n\nOne one.\n\nTwo two.",
            ],
        );
        assert_eq!(
            blame(&h, 1),
            "Three@0 three@0 .@0 One@0 one@0 .@0 Two@0 two@0 .@0"
        );
        // Out-of-order `old` positions: ribbons that cross.
        assert_eq!(h.links(0, 1), [link(6, 0, 3), link(0, 3, 6)]);
    }

    #[test]
    fn historical_restores_a_single_sentence_inside_an_edited_paragraph() {
        let h = build(
            Config::prose(),
            &[
                "Keep this. Drop that.",
                "Keep this.",
                "Keep this. New words. Drop that.",
            ],
        );
        assert_eq!(
            blame(&h, 2),
            "Keep@0 this@0 .@0 New@2 words@2 .@2 Drop@0 that@0 .@0"
        );
    }

    #[test]
    fn a_duplicated_passage_is_one_original_and_one_new_copy() {
        let h = build(
            Config::prose(),
            &[
                "The very same words.",
                "The very same words.\n\nThe very same words.",
            ],
        );
        assert_eq!(
            blame(&h, 1),
            "The@0 very@0 same@0 words@0 .@0 The@1 very@1 same@1 words@1 .@1"
        );
        // And the copy is itself remembered: both survive a revert.
        let h = build(
            Config::prose(),
            &[
                "The very same words.",
                "The very same words.\n\nThe very same words.",
                "x",
                "The very same words.\n\nThe very same words.",
            ],
        );
        assert_eq!(blame(&h, 3), blame(&h, 1));
    }

    #[test]
    fn sentences_change_whole_when_a_comma_is_added() {
        let h = build(
            Config::sentences_2004(),
            &["First one. Second one.", "First, one. Second one."],
        );
        assert_eq!(blame(&h, 1), "First, one.@1 Second one.@0");
    }

    #[test]
    fn source_lines_blame_like_git() {
        let v0 = "fn a() {\n    one();\n}\n";
        let v1 = "fn a() {\n    one();\n}\n\nfn b() {\n    two();\n}\n";
        let v2 = "fn a() {\n    one();\n    more();\n}\n\nfn b() {\n}\n";
        let h = build(Config::source_lines(), &[v0, v1, v2]);
        assert_eq!(
            blame(&h, 2),
            "fn a() {@0     one();@0     more();@2 }@0 @1 fn b() {@1 }@1"
        );
        // Measured in lines.
        assert_eq!(h.versions()[2].weight(), 7);
    }

    #[test]
    fn source_lines_treat_an_indentation_change_as_a_change() {
        let h = build(Config::source_lines(), &["x\n", "  x\n"]);
        assert_eq!(blame(&h, 1), "  x@1");
    }

    #[test]
    fn authors_are_shared_between_versions() {
        let mut builder = HistoryBuilder::new(Config::prose());
        for (author, anonymous) in [("ann", false), ("10.0.0.1", true), ("ann", false)] {
            builder.push(&SourceVersion {
                author: author.into(),
                anonymous,
                text: author.into(),
                ..SourceVersion::default()
            });
        }
        let h = builder.finish();
        assert_eq!(h.authors().len(), 2);
        assert!(h.authors()[1].anonymous);
        assert_eq!(h.versions()[2].author, h.versions()[0].author);
    }

    /// Random edit histories, checked against the properties everything
    /// downstream relies on.
    #[test]
    fn random_histories_keep_the_invariants() {
        let words = ["a", "b", "c", "d", "e", "f", ".", "\n", "\n\n"];
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        let mut random = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        for config in [Config::prose, Config::sentences_2004, Config::source_lines] {
            let mut doc: Vec<&str> = Vec::new();
            let mut snapshots: Vec<String> = Vec::new();
            let mut builder = HistoryBuilder::new(config());
            for _ in 0..300 {
                match random(10) {
                    // Revert to an earlier version.
                    0 if !snapshots.is_empty() => {
                        let text = snapshots[random(snapshots.len())].clone();
                        builder.push(&SourceVersion {
                            text: text.clone(),
                            ..Default::default()
                        });
                        snapshots.push(text);
                        continue;
                    }
                    // Delete a stretch.
                    1 | 2 if !doc.is_empty() => {
                        let start = random(doc.len());
                        let end = (start + random(8)).min(doc.len());
                        doc.drain(start..end);
                    }
                    // Move a stretch.
                    3 if doc.len() > 4 => {
                        let start = random(doc.len() - 3);
                        let moved: Vec<_> = doc.drain(start..start + 3).collect();
                        let to = random(doc.len() + 1);
                        doc.splice(to..to, moved);
                    }
                    // Insert a stretch.
                    _ => {
                        let at = random(doc.len() + 1);
                        let added: Vec<_> = (0..1 + random(6))
                            .map(|_| words[random(words.len())])
                            .collect();
                        doc.splice(at..at, added);
                    }
                }
                let text = doc.join(" ");
                builder.push(&SourceVersion {
                    text: text.clone(),
                    ..Default::default()
                });
                snapshots.push(text);
            }

            let segmenter = &builder.config().segmenter;
            let h = builder.history();
            for (index, version) in h.versions().iter().enumerate() {
                let text = &snapshots[index];
                let tokens: Vec<_> = version.tokens().collect();
                // One token per atom, carrying that atom's text.
                let atoms = segmenter.segment(text).atoms;
                assert_eq!(tokens.len(), atoms.len());
                for (token, atom) in tokens.iter().zip(atoms) {
                    assert_eq!(h.text(*token), &text[atom]);
                    assert!(h.origin(*token) as usize <= index);
                }
                // No token twice in a version.
                let mut sorted = tokens.clone();
                sorted.sort();
                sorted.dedup();
                assert_eq!(sorted.len(), tokens.len());
                // Runs are maximal.
                assert!(
                    version
                        .runs()
                        .windows(2)
                        .all(|w| w[0].first.0 + w[0].len != w[1].first.0)
                );

                // Links to the previous version are exactly the shared
                // tokens, as maximal runs.
                if index > 0 {
                    let previous: Vec<_> = h.versions()[index - 1].tokens().collect();
                    let links = h.links(index as u32 - 1, index as u32);
                    let mut linked = 0;
                    for l in &links {
                        let (o, n, len) = (l.old as usize, l.new as usize, l.len as usize);
                        assert_eq!(previous[o..o + len], tokens[n..n + len]);
                        let extends_back = o > 0 && n > 0 && previous[o - 1] == tokens[n - 1];
                        assert!(!extends_back);
                        linked += len;
                    }
                    let shared = tokens.iter().filter(|t| previous.contains(t)).count();
                    assert_eq!(linked, shared);
                }
            }
        }
    }
}

#[cfg(test)]
mod repeated_passage_tests {
    use super::*;

    #[test]
    fn identical_passages_keep_their_order() {
        // Three "See also." paragraphs around one that is edited. Each
        // must stay where it was: two ribbons, not crossing.
        let before = "See also.\n\nA longer paragraph of text here.\n\nSee also.\n\nSee also.";
        let after = "See also.\n\nA longer paragraph of new text here.\n\nSee also.\n\nSee also.";
        let mut builder = HistoryBuilder::new(Config::prose());
        for text in [before, after] {
            builder.push(&SourceVersion {
                text: text.into(),
                ..Default::default()
            });
        }
        let h = builder.finish();
        let links = h.links(0, 1);
        assert_eq!(links.len(), 2, "{links:?}");
        assert!(links.windows(2).all(|w| w[0].old < w[1].old));
    }
}
