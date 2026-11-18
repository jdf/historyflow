//! A version's text divided by who wrote what, for display beside the
//! plot.

use std::ops::Range;

use crate::model::{History, VersionIndex};
use crate::segment::Segmenter;

/// A stretch of one version's text introduced by a single version.
#[derive(Debug, Clone, PartialEq)]
pub struct Passage {
    /// Byte range of the version's text. Passages are contiguous and
    /// together cover the whole text: the whitespace between atoms
    /// belongs to the passage before it (or, at the very start, after).
    pub text: Range<usize>,
    /// The version that introduced it.
    pub origin: VersionIndex,
    /// Where it sits on the version's line in the plot: top and bottom, in
    /// the layout's y.
    pub y: [f32; 2],
}

/// Splits `text`, which must be the text version `version` was built
/// from, into passages. `segmenter` must be the one the history was
/// built with.
///
/// # Panics
///
/// If the text does not segment into as many atoms as the version has
/// tokens, which means it is not that version's text.
pub fn passages(
    history: &History,
    version: VersionIndex,
    text: &str,
    segmenter: &dyn Segmenter,
) -> Vec<Passage> {
    let atoms = segmenter.segment(text).atoms;
    let tokens: Vec<_> = history.versions()[version as usize].tokens().collect();
    assert_eq!(atoms.len(), tokens.len(), "text does not match the version");
    let tallest = history.tallest() as f64;

    let mut passages: Vec<Passage> = Vec::new();
    let mut weight = 0u64;
    for (atom, &token) in atoms.iter().zip(&tokens) {
        let top = (weight as f64 / tallest) as f32;
        weight += u64::from(history.weight(token));
        let bottom = (weight as f64 / tallest) as f32;
        let origin = history.origin(token);
        match passages.last_mut() {
            Some(last) if last.origin == origin => {
                last.text.end = atom.end;
                last.y[1] = bottom;
            }
            last => {
                // The gap before this atom goes to the passage before.
                let start = match last {
                    Some(last) => {
                        last.text.end = atom.start;
                        atom.start
                    }
                    None => 0,
                };
                passages.push(Passage {
                    text: start..atom.end,
                    origin,
                    y: [top, bottom],
                });
            }
        }
    }
    match passages.last_mut() {
        Some(last) => last.text.end = text.len(),
        // Nothing but whitespace: the version's own, with no extent.
        None if !text.is_empty() => passages.push(Passage {
            text: 0..text.len(),
            origin: version,
            y: [0.0, 0.0],
        }),
        None => {}
    }
    passages
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, HistoryBuilder, SourceVersion};

    fn build(config: Config, texts: &[&str]) -> (History, Box<dyn Segmenter>) {
        let mut builder = HistoryBuilder::new(config);
        for text in texts {
            builder.push(&SourceVersion {
                text: text.to_string(),
                ..Default::default()
            });
        }
        let segmenter = Config::prose().segmenter;
        (builder.finish(), segmenter)
    }

    fn show(passages: &[Passage], text: &str) -> Vec<(String, u32, [f32; 2])> {
        passages
            .iter()
            .map(|p| (text[p.text.clone()].to_string(), p.origin, p.y))
            .collect()
    }

    #[test]
    fn passages_cover_the_text_and_carry_origin_and_extent() {
        let texts = ["  aa bb.", "  aa cccc bb.\n"];
        let (h, segmenter) = build(Config::prose(), &texts);
        // Weights: aa 2, cccc 4, bb 2, "." 1; the tallest version is 9.
        let n = |w: f32| w / 9.0;
        assert_eq!(
            show(&passages(&h, 1, texts[1], &*segmenter), texts[1]),
            [
                ("  aa ".to_string(), 0, [0.0, n(2.0)]),
                ("cccc ".to_string(), 1, [n(2.0), n(6.0)]),
                ("bb.\n".to_string(), 0, [n(6.0), n(9.0)]),
            ]
        );
        assert_eq!(
            show(&passages(&h, 0, texts[0], &*segmenter), texts[0]),
            [("  aa bb.".to_string(), 0, [0.0, n(5.0)])]
        );
    }

    #[test]
    fn text_with_no_atoms_is_one_passage_or_none() {
        let texts = ["word", " \n", ""];
        let (h, segmenter) = build(Config::prose(), &texts);
        assert_eq!(
            show(&passages(&h, 1, texts[1], &*segmenter), texts[1]),
            [(" \n".to_string(), 1, [0.0, 0.0])]
        );
        assert_eq!(passages(&h, 2, texts[2], &*segmenter), []);
    }
}
