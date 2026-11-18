//! Turning a history into geometry.
//!
//! Coordinates are normalised: x runs 0–1 from the first version to the
//! last, y runs 0–1 from the top of the document to the end of the
//! longest version. Every version starts at y = 0, so the empty space
//! under the plot traces the document's length over time.

use crate::model::{Author, History, TokenId, VersionIndex};

/// How versions are placed along x.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Spacing {
    /// Evenly.
    #[default]
    Version,
    /// In proportion to the time between them, which collapses bursts of
    /// rapid edits (and makes quickly repaired vandalism vanish).
    Date,
}

/// One filled quadrilateral: a stretch of text by one author, either
/// within a version's line (`x0 == x1`, and the left and right sides are
/// the same) or connecting the same text in two consecutive versions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quad {
    pub x0: f32,
    pub x1: f32,
    /// Top and bottom on the left side.
    pub left: [f32; 2],
    /// Top and bottom on the right side.
    pub right: [f32; 2],
    /// Index into [`History::authors`].
    pub author: u32,
    /// How many versions old the text is on the left and right sides.
    pub age: [f32; 2],
}

impl Quad {
    /// The number of floats in the flattened form.
    pub const FLOATS: usize = 9;

    /// `x0, x1, left top, left bottom, right top, right bottom, author,
    /// left age, right age`: the per-instance vertex data.
    pub fn floats(&self) -> [f32; Quad::FLOATS] {
        [
            self.x0,
            self.x1,
            self.left[0],
            self.left[1],
            self.right[0],
            self.right[1],
            self.author as f32,
            self.age[0],
            self.age[1],
        ]
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Layout {
    /// The x of each version's line.
    pub xs: Vec<f32>,
    /// Version lines and the ribbons between them, in no particular
    /// order; none overlap.
    pub quads: Vec<Quad>,
}

/// A version's tokens with the y at which each starts (and, last, the y
/// at which the version ends).
struct Column {
    tokens: Vec<TokenId>,
    ys: Vec<f32>,
}

pub fn layout(history: &History, spacing: Spacing) -> Layout {
    let versions = history.versions();
    let xs = positions(history, spacing);
    let tallest = history.tallest() as f64;

    let mut quads = Vec::new();
    let mut previous: Option<Column> = None;
    for (index, version) in versions.iter().enumerate() {
        let tokens: Vec<TokenId> = version.tokens().collect();
        let mut ys = Vec::with_capacity(tokens.len() + 1);
        let mut total = 0u64;
        ys.push(0.0);
        for &token in &tokens {
            total += u64::from(history.weight(token));
            ys.push((total as f64 / tallest) as f32);
        }
        let column = Column { tokens, ys };
        let x = xs[index];
        let age = |token: TokenId| (index as VersionIndex - history.origin(token)) as f32;

        // The version's own line.
        for (start, end) in same_origin(history, &column.tokens, 0, column.tokens.len()) {
            let side = [column.ys[start], column.ys[end]];
            if side[0] == side[1] {
                continue;
            }
            let token = column.tokens[start];
            quads.push(Quad {
                x0: x,
                x1: x,
                left: side,
                right: side,
                author: history.author(token).0,
                age: [age(token); 2],
            });
        }

        // Ribbons from the previous version.
        if let Some(old) = &previous {
            for link in history.links(index as VersionIndex - 1, index as VersionIndex) {
                let (from, to) = (link.new as usize, (link.new + link.len) as usize);
                let shift = link.old as usize;
                for (start, end) in same_origin(history, &column.tokens, from, to) {
                    let right = [column.ys[start], column.ys[end]];
                    if right[0] == right[1] {
                        continue;
                    }
                    let token = column.tokens[start];
                    quads.push(Quad {
                        x0: xs[index - 1],
                        x1: x,
                        left: [old.ys[start - from + shift], old.ys[end - from + shift]],
                        right,
                        author: history.author(token).0,
                        age: [age(token) - 1.0, age(token)],
                    });
                }
            }
        }
        previous = Some(column);
    }
    Layout { xs, quads }
}

/// Splits `tokens[from..to]` into maximal stretches introduced by the
/// same version, which share an author and an age.
fn same_origin<'a>(
    history: &'a History,
    tokens: &'a [TokenId],
    from: usize,
    to: usize,
) -> impl Iterator<Item = (usize, usize)> + 'a {
    let mut start = from;
    std::iter::from_fn(move || {
        if start >= to {
            return None;
        }
        let origin = history.origin(tokens[start]);
        let mut end = start + 1;
        while end < to && history.origin(tokens[end]) == origin {
            end += 1;
        }
        let stretch = (start, end);
        start = end;
        Some(stretch)
    })
}

fn positions(history: &History, spacing: Spacing) -> Vec<f32> {
    let versions = history.versions();
    let count = versions.len();
    if count <= 1 {
        return vec![0.5; count];
    }
    let even = || (0..count).map(|i| i as f32 / (count - 1) as f32).collect();
    match spacing {
        Spacing::Version => even(),
        Spacing::Date => {
            // Timestamps can run backwards (a rebased commit, a clock
            // error); a version is never placed left of its predecessor.
            let mut times = Vec::with_capacity(count);
            let mut latest = i64::MIN;
            for version in versions {
                latest = latest.max(version.timestamp);
                times.push(latest);
            }
            let span = times[count - 1] - times[0];
            if span == 0 {
                return even();
            }
            times
                .iter()
                .map(|&t| ((t - times[0]) as f64 / span as f64) as f32)
                .collect()
        }
    }
}

/// A colour for each author, as RGB in 0–1. Named authors get distinct
/// light, saturated hues in order of first appearance; anonymous authors
/// get white and greys, as in the original.
pub fn author_colors(authors: &[Author]) -> Vec<[f32; 3]> {
    const GREYS: [f32; 5] = [1.0, 0.78, 0.6, 0.9, 0.69];
    let (mut named, mut anonymous) = (0, 0);
    authors
        .iter()
        .map(|author| {
            if author.anonymous {
                anonymous += 1;
                [GREYS[(anonymous - 1) % GREYS.len()]; 3]
            } else {
                named += 1;
                // Golden-angle steps keep neighbouring authors far apart
                // in hue however many there are.
                let hue = ((named - 1) as f32 * 137.508 + 25.0) % 360.0;
                hsv(hue, 0.62, 1.0)
            }
        })
        .collect()
}

fn hsv(hue: f32, saturation: f32, value: f32) -> [f32; 3] {
    let channel = |n: f32| {
        let k = (n + hue / 60.0) % 6.0;
        value - value * saturation * k.min(4.0 - k).clamp(0.0, 1.0)
    };
    [channel(5.0), channel(3.0), channel(1.0)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, HistoryBuilder, SourceVersion};

    fn history(versions: &[(i64, &str)]) -> History {
        let mut builder = HistoryBuilder::new(Config::source_lines());
        for (i, &(timestamp, text)) in versions.iter().enumerate() {
            builder.push(&SourceVersion {
                author: format!("author {i}"),
                timestamp,
                text: text.into(),
                ..Default::default()
            });
        }
        builder.finish()
    }

    fn quad(x: [f32; 2], left: [f32; 2], right: [f32; 2], author: u32, age: [f32; 2]) -> Quad {
        Quad {
            x0: x[0],
            x1: x[1],
            left,
            right,
            author,
            age,
        }
    }

    #[test]
    fn lines_and_ribbons_of_an_insertion_and_a_deletion() {
        // v0: a b      v1: a X b c      v2: X b c
        let h = history(&[(0, "a\nb\n"), (10, "a\nX\nb\nc\n"), (20, "X\nb\nc\n")]);
        let l = layout(&h, Spacing::Version);
        assert_eq!(l.xs, [0.0, 0.5, 1.0]);
        let q = |n: f32| n / 4.0;
        assert_eq!(
            l.quads,
            [
                // v0's line: a b, all its own.
                quad([0.0, 0.0], [0.0, q(2.0)], [0.0, q(2.0)], 0, [0.0, 0.0]),
                // v1's line: a | X | b | c.
                quad([0.5, 0.5], [0.0, q(1.0)], [0.0, q(1.0)], 0, [1.0, 1.0]),
                quad(
                    [0.5, 0.5],
                    [q(1.0), q(2.0)],
                    [q(1.0), q(2.0)],
                    1,
                    [0.0, 0.0]
                ),
                quad(
                    [0.5, 0.5],
                    [q(2.0), q(3.0)],
                    [q(2.0), q(3.0)],
                    0,
                    [1.0, 1.0]
                ),
                quad(
                    [0.5, 0.5],
                    [q(3.0), q(4.0)],
                    [q(3.0), q(4.0)],
                    1,
                    [0.0, 0.0]
                ),
                // v0 → v1: a stays put, b is pushed down by the insertion.
                quad([0.0, 0.5], [0.0, q(1.0)], [0.0, q(1.0)], 0, [0.0, 1.0]),
                quad(
                    [0.0, 0.5],
                    [q(1.0), q(2.0)],
                    [q(2.0), q(3.0)],
                    0,
                    [0.0, 1.0]
                ),
                // v2's line: X | b | c.
                quad([1.0, 1.0], [0.0, q(1.0)], [0.0, q(1.0)], 1, [1.0, 1.0]),
                quad(
                    [1.0, 1.0],
                    [q(1.0), q(2.0)],
                    [q(1.0), q(2.0)],
                    0,
                    [2.0, 2.0]
                ),
                quad(
                    [1.0, 1.0],
                    [q(2.0), q(3.0)],
                    [q(2.0), q(3.0)],
                    1,
                    [1.0, 1.0]
                ),
                // v1 → v2: one link, X b c rising into a's place, in
                // three pieces by origin.
                quad([0.5, 1.0], [q(1.0), q(2.0)], [0.0, q(1.0)], 1, [0.0, 1.0]),
                quad(
                    [0.5, 1.0],
                    [q(2.0), q(3.0)],
                    [q(1.0), q(2.0)],
                    0,
                    [1.0, 2.0]
                ),
                quad(
                    [0.5, 1.0],
                    [q(3.0), q(4.0)],
                    [q(2.0), q(3.0)],
                    1,
                    [0.0, 1.0]
                ),
            ]
        );
    }

    #[test]
    fn date_spacing_follows_time_and_never_goes_backwards() {
        let h = history(&[(100, "a\n"), (110, "b\n"), (105, "c\n"), (200, "d\n")]);
        assert_eq!(layout(&h, Spacing::Date).xs, [0.0, 0.1, 0.1, 1.0]);
    }

    #[test]
    fn date_spacing_falls_back_to_even_when_no_time_passes() {
        let h = history(&[(5, "a\n"), (5, "b\n"), (5, "c\n")]);
        assert_eq!(layout(&h, Spacing::Date).xs, [0.0, 0.5, 1.0]);
    }

    #[test]
    fn a_single_version_is_centred_and_an_empty_history_is_empty() {
        assert_eq!(layout(&history(&[(0, "a\n")]), Spacing::Date).xs, [0.5]);
        assert_eq!(layout(&history(&[]), Spacing::Version), Layout::default());
        assert_eq!(layout(&history(&[]), Spacing::Date), Layout::default());
    }

    #[test]
    fn anonymous_authors_are_grey_and_named_authors_differ() {
        let author = |name: &str, anonymous| Author {
            name: name.into(),
            anonymous,
        };
        let colors = author_colors(&[author("a", false), author("ip", true), author("b", false)]);
        assert_eq!(colors[1], [1.0, 1.0, 1.0]);
        assert_ne!(colors[0], colors[2]);
        for c in [colors[0], colors[2]] {
            assert!(c.iter().all(|v| (0.0..=1.0).contains(v)));
            assert!(c.iter().cloned().fold(0.0, f32::max) == 1.0);
        }
    }
}
