//! Splitting a version's text into the pieces that are compared.

use std::ops::Range;

/// A text split into three nested levels: blocks contain units, units
/// contain atoms. Atoms are the pieces that get token identities; units
/// and blocks exist so that [`Matching::Historical`] can recognise larger
/// unchanged or restored passages by hash before comparing atoms.
///
/// Every atom belongs to exactly one unit and every unit to exactly one
/// block. Atoms are byte ranges of the text, in order and non-overlapping;
/// text between atoms (whitespace, line terminators) is not compared.
///
/// [`Matching::Historical`]: crate::Matching::Historical
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Segmentation {
    /// Byte range of each atom.
    pub atoms: Vec<Range<usize>>,
    /// For each unit, the index one past its last atom.
    pub unit_ends: Vec<u32>,
    /// For each block, the index one past its last unit.
    pub block_ends: Vec<u32>,
}

impl Segmentation {
    /// The atom-index range of each unit.
    pub fn units(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        ranges(&self.unit_ends)
    }

    /// The unit-index range of each block.
    pub fn blocks(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        ranges(&self.block_ends)
    }
}

fn ranges(ends: &[u32]) -> impl Iterator<Item = Range<usize>> + '_ {
    let mut start = 0;
    ends.iter().map(move |&end| {
        let range = start..end as usize;
        start = end as usize;
        range
    })
}

/// Splits text into a [`Segmentation`]. Must be deterministic: the same
/// text always yields the same result, so a caller can re-segment a
/// version later to line its atoms up with the tokens recorded for it.
pub trait Segmenter {
    fn segment(&self, text: &str) -> Segmentation;
}

/// Accumulates a segmentation, closing units and blocks on request and
/// never emitting an empty one.
#[derive(Default)]
struct Builder {
    out: Segmentation,
    unit_start: usize,
    block_start: usize,
}

impl Builder {
    fn atom(&mut self, range: Range<usize>) {
        self.out.atoms.push(range);
    }

    fn end_unit(&mut self) {
        if self.out.atoms.len() > self.unit_start {
            self.out.unit_ends.push(self.out.atoms.len() as u32);
            self.unit_start = self.out.atoms.len();
        }
    }

    fn end_block(&mut self) {
        self.end_unit();
        if self.out.unit_ends.len() > self.block_start {
            self.out.block_ends.push(self.out.unit_ends.len() as u32);
            self.block_start = self.out.unit_ends.len();
        }
    }

    fn finish(mut self) -> Segmentation {
        self.end_block();
        self.out
    }
}

/// Words within sentences within paragraphs, for prose and wiki markup.
///
/// - An atom is a run of alphanumeric characters, or any single other
///   non-whitespace character. Markup such as `[[` therefore becomes
///   tokens of its own, and whitespace changes are invisible.
/// - A unit (sentence) ends at a line break, or after `.`, `?`, `!`, `;`
///   or `:` when whitespace follows.
/// - A block (paragraph) ends at a blank line.
///
/// This follows WikiWho's hierarchy without its wikitext-specific rules
/// for references, tables and comments.
#[derive(Debug, Default, Clone, Copy)]
pub struct Prose;

impl Segmenter for Prose {
    fn segment(&self, text: &str) -> Segmentation {
        let mut b = Builder::default();
        let mut word_start = None;
        let mut line_has_atoms = false;
        let mut after_terminator = false;
        for (i, c) in text.char_indices() {
            if c.is_alphanumeric() {
                word_start.get_or_insert(i);
                continue;
            }
            if let Some(start) = word_start.take() {
                b.atom(start..i);
                line_has_atoms = true;
                after_terminator = false;
            }
            if c.is_whitespace() {
                if after_terminator {
                    b.end_unit();
                    after_terminator = false;
                }
                if c == '\n' {
                    if line_has_atoms {
                        b.end_unit();
                    } else {
                        b.end_block();
                    }
                    line_has_atoms = false;
                }
            } else {
                b.atom(i..i + c.len_utf8());
                line_has_atoms = true;
                after_terminator = matches!(c, '.' | '?' | '!' | ';' | ':');
            }
        }
        if let Some(start) = word_start {
            b.atom(start..text.len());
        }
        b.finish()
    }
}

/// The original 2004 tokenizer: "sentences" delimited by periods and
/// HTML tags, with no finer level. Changing one comma changes the whole
/// sentence, as the original's authors noted.
///
/// - An atom ends after `.` or `>`, before `<`, and at a line break;
///   surrounding whitespace is trimmed and empty pieces are dropped.
/// - Each atom is its own unit.
/// - A block is a line.
///
/// The published descriptions give only the delimiters; attaching each
/// delimiter to the piece before it, and splitting at line breaks, are
/// our choices.
#[derive(Debug, Default, Clone, Copy)]
pub struct Sentences;

impl Segmenter for Sentences {
    fn segment(&self, text: &str) -> Segmentation {
        let mut b = Builder::default();
        let mut start = 0;
        let flush = |b: &mut Builder, start: usize, end: usize| {
            let piece = &text[start..end];
            let trimmed = piece.trim();
            if !trimmed.is_empty() {
                let offset = start + (piece.len() - piece.trim_start().len());
                b.atom(offset..offset + trimmed.len());
                b.end_unit();
            }
        };
        for (i, c) in text.char_indices() {
            match c {
                '.' | '>' => {
                    flush(&mut b, start, i + 1);
                    start = i + 1;
                }
                '<' => {
                    flush(&mut b, start, i);
                    start = i;
                }
                '\n' => {
                    flush(&mut b, start, i);
                    start = i + 1;
                    b.end_block();
                }
                _ => {}
            }
        }
        flush(&mut b, start, text.len());
        b.finish()
    }
}

/// Whole lines, for source code.
///
/// - An atom is one line without its terminator (`\n` or `\r\n`). Blank
///   lines are atoms too. Nothing inside a line is normalised: a change
///   of indentation is a change to the line.
/// - Each line is its own unit, and the whole text is one block.
#[derive(Debug, Default, Clone, Copy)]
pub struct Lines;

impl Segmenter for Lines {
    fn segment(&self, text: &str) -> Segmentation {
        let mut b = Builder::default();
        let mut start = 0;
        while start < text.len() {
            let end = text[start..].find('\n').map_or(text.len(), |n| start + n);
            let content_end = if text[start..end].ends_with('\r') {
                end - 1
            } else {
                end
            };
            b.atom(start..content_end);
            b.end_unit();
            start = end + 1;
        }
        b.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The atoms' text, with `|` between units and `‖` between blocks.
    fn show(segmenter: &dyn Segmenter, text: &str) -> String {
        let seg = segmenter.segment(text);
        check_nesting(&seg, text);
        let units: Vec<_> = seg.units().collect();
        seg.blocks()
            .map(|block| {
                units[block]
                    .iter()
                    .map(|unit| {
                        seg.atoms[unit.clone()]
                            .iter()
                            .map(|atom| &text[atom.clone()])
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .collect::<Vec<_>>()
            .join(" ‖ ")
    }

    fn check_nesting(seg: &Segmentation, text: &str) {
        assert_eq!(
            seg.unit_ends.last().copied().unwrap_or(0) as usize,
            seg.atoms.len()
        );
        assert_eq!(
            seg.block_ends.last().copied().unwrap_or(0) as usize,
            seg.unit_ends.len()
        );
        assert!(seg.unit_ends.windows(2).all(|w| w[0] < w[1]));
        assert!(seg.block_ends.windows(2).all(|w| w[0] < w[1]));
        let mut previous_end = 0;
        for atom in &seg.atoms {
            assert!(atom.start >= previous_end && atom.end <= text.len());
            previous_end = atom.end;
        }
    }

    #[test]
    fn prose_splits_words_sentences_and_paragraphs() {
        assert_eq!(
            show(&Prose, "One two. Three!\nFour\n\n  \nFive [[six]]"),
            "One two . | Three ! | Four ‖ Five [ [ six ] ]"
        );
    }

    #[test]
    fn prose_keeps_a_terminator_inside_a_word_from_ending_the_sentence() {
        assert_eq!(show(&Prose, "pi is 3.14 ok"), "pi is 3 . 14 ok");
    }

    #[test]
    fn prose_handles_non_ascii_and_empty_text() {
        assert_eq!(show(&Prose, "Viégas – naïve"), "Viégas – naïve");
        assert_eq!(show(&Prose, ""), "");
        assert_eq!(show(&Prose, " \n\n "), "");
    }

    #[test]
    fn sentences_split_on_periods_and_tags() {
        assert_eq!(
            show(&Sentences, "A b. C, d.<br>E <i>f</i>\ng"),
            "A b. | C, d. | <br> | E | <i> | f | </i> ‖ g"
        );
    }

    #[test]
    fn lines_keep_blank_lines_and_drop_terminators() {
        let seg = Lines.segment("a\r\n\n  b\n");
        let text = "a\r\n\n  b\n";
        let atoms: Vec<_> = seg.atoms.iter().map(|a| &text[a.clone()]).collect();
        assert_eq!(atoms, ["a", "", "  b"]);
        assert_eq!(seg.unit_ends, [1, 2, 3]);
        assert_eq!(seg.block_ends, [3]);
    }

    #[test]
    fn lines_without_a_final_newline() {
        assert_eq!(show(&Lines, "a\nb"), "a | b");
        assert_eq!(show(&Lines, ""), "");
    }
}
