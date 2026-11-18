//! A compact, lossless store of every version of a document.
//!
//! A [`Bundle`](crate::Bundle) holds each version's whole text, so its
//! size is the document's size times the number of versions. An archive
//! holds only what each version changed:
//!
//! - Every distinct *chunk* of text (a word, a run of spaces, a
//!   punctuation mark, a line break) is stored once, however many times
//!   and in however many versions it occurs.
//! - A version is a list of instructions for making its sequence of
//!   chunks out of an earlier version's: copy this stretch, insert these
//!   chunks. The earlier version is usually the one before; it is the
//!   one before that when that is nearer (an edit reverted, two authors
//!   alternating), and any earlier version that was exactly the same (a
//!   page restored after vandalism), which costs a few bytes.
//! - The whole is then deflated.
//!
//! Chunks partition the text exactly, so any version's text comes back
//! byte for byte. Nothing here knows about authorship: an archive is
//! read back into text, and the text is attributed as from any source,
//! in whatever [`Mode`] is wanted.
//!
//! In memory an archive is the same instructions, plus the chunk
//! sequences of a few versions (one in every [`CHECKPOINT`] along each
//! chain) so that any version's text is at most that many steps away.

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use imara_diff::{Algorithm, Diff, Token};

use crate::bundle::{Bundle, Mode};
use crate::model::{Author, SourceVersion};

/// The first bytes of an archive file.
pub const MAGIC: &[u8] = b"historyflow-archive/1\n";

/// How many steps of instructions lie between the versions whose chunk
/// sequences are kept whole in memory.
const CHECKPOINT: u32 = 64;

/// The longest chunk sequence a version may have. Far beyond any real
/// document; it stops a malformed file from asking for unbounded memory.
const MAX_CHUNKS: usize = 1 << 26;

/// The most an archive may inflate to, for the same reason.
const MAX_INFLATED: u64 = 1 << 32;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Op {
    /// `len` chunks of the base version, starting at `start`.
    Copy { start: u32, len: u32 },
    /// These chunks, by id.
    Insert(Vec<u32>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry {
    id: String,
    author: u32,
    timestamp: i64,
    comment: String,
    /// The version the instructions start from, or `None` to start from
    /// nothing.
    base: Option<u32>,
    ops: Vec<Op>,
}

#[derive(Debug, Default)]
pub struct Archive {
    pub title: String,
    /// The comparison the source suggests; the viewer may override it.
    pub mode: Mode,
    authors: Vec<Author>,
    chunks: Vec<Box<str>>,
    entries: Vec<Entry>,

    // Everything below is derived from the above, to make adding and
    // reading versions fast; none of it is stored.
    author_ids: HashMap<Author, u32>,
    chunk_ids: HashMap<Box<str>, u32>,
    /// Per version: the length of its chain of bases, and its length in
    /// chunks and in bytes of text.
    depths: Vec<u32>,
    lens: Vec<u32>,
    text_lens: Vec<u64>,
    /// The whole chunk sequences of some versions.
    checkpoints: HashMap<u32, Vec<u32>>,
    /// The sequences of the last two versions, newest first.
    recent: Vec<(u32, Vec<u32>)>,
    /// The first version with each sequence, by its hash.
    by_hash: HashMap<u64, u32>,
    diff: Diff,
}

impl Archive {
    pub fn new(title: impl Into<String>, mode: Mode) -> Archive {
        Archive {
            title: title.into(),
            mode,
            ..Archive::default()
        }
    }

    /// Reads a history from the bytes of a file of either kind: an
    /// archive, or a bundle's JSON.
    pub fn read(bytes: &[u8]) -> Result<Archive, String> {
        if Archive::is_archive(bytes) {
            return Archive::from_bytes(bytes);
        }
        let bundle: Bundle = serde_json::from_slice(bytes)
            .map_err(|e| format!("neither a history flow archive nor a bundle: {e}"))?;
        if bundle.format != Bundle::FORMAT {
            return Err(format!("unsupported bundle format {:?}", bundle.format));
        }
        Ok(Archive::from_bundle(&bundle))
    }

    pub fn from_bundle(bundle: &Bundle) -> Archive {
        let mut archive = Archive::new(bundle.title.clone(), bundle.mode);
        for version in &bundle.versions {
            archive.push(version);
        }
        archive
    }

    /// The same history with every version's text written out.
    pub fn to_bundle(&self) -> Bundle {
        Bundle::new(self.title.clone(), self.mode, self.versions().collect())
    }

    /// The number of versions.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The size of all the versions' text together: what a bundle of the
    /// same history would hold.
    pub fn text_bytes(&self) -> u64 {
        self.text_lens.iter().sum()
    }

    /// Adds the next version.
    pub fn push(&mut self, source: &SourceVersion) {
        let index = self.entries.len() as u32;
        let sequence: Vec<u32> = chunks(&source.text)
            .map(|chunk| self.intern(chunk))
            .collect();

        let (base, ops) = self.encode(&sequence);
        let author = Author {
            name: source.author.clone(),
            anonymous: source.anonymous,
        };
        let author = match self.author_ids.get(&author) {
            Some(&id) => id,
            None => {
                let id = self.authors.len() as u32;
                self.authors.push(author.clone());
                self.author_ids.insert(author, id);
                id
            }
        };
        self.entries.push(Entry {
            id: source.id.clone(),
            author,
            timestamp: source.timestamp,
            comment: source.comment.clone(),
            base,
            ops,
        });
        self.admit(index, sequence, source.text.len() as u64);
    }

    /// One version, text and all.
    pub fn version(&self, index: usize) -> SourceVersion {
        self.source(index, &self.sequence(index as u32))
    }

    /// One version's text.
    pub fn text(&self, index: usize) -> String {
        self.join(&self.sequence(index as u32))
    }

    /// Every version in order. Faster than asking for each in turn, since
    /// each is made from the one just made.
    pub fn versions(&self) -> impl Iterator<Item = SourceVersion> + '_ {
        let mut recent: Vec<(u32, Vec<u32>)> = Vec::new();
        (0..self.entries.len()).map(move |index| {
            let sequence = self.next_sequence(index as u32, &recent);
            let version = self.source(index, &sequence);
            recent.insert(0, (index as u32, sequence));
            recent.truncate(2);
            version
        })
    }

    fn source(&self, index: usize, sequence: &[u32]) -> SourceVersion {
        let entry = &self.entries[index];
        let author = &self.authors[entry.author as usize];
        SourceVersion {
            id: entry.id.clone(),
            author: author.name.clone(),
            anonymous: author.anonymous,
            timestamp: entry.timestamp,
            comment: entry.comment.clone(),
            text: self.join(sequence),
        }
    }

    fn join(&self, sequence: &[u32]) -> String {
        sequence
            .iter()
            .map(|&chunk| &*self.chunks[chunk as usize])
            .collect()
    }

    fn intern(&mut self, chunk: &str) -> u32 {
        if let Some(&id) = self.chunk_ids.get(chunk) {
            return id;
        }
        let id = self.chunks.len() as u32;
        self.chunks.push(chunk.into());
        self.chunk_ids.insert(chunk.into(), id);
        id
    }

    /// Chooses how to make `sequence` from an earlier version.
    fn encode(&mut self, sequence: &[u32]) -> (Option<u32>, Vec<Op>) {
        // The same as an earlier version: copy all of it.
        if let Some(&same) = self.by_hash.get(&hash(sequence))
            && self.sequence(same) == sequence
        {
            let all = Op::Copy {
                start: 0,
                len: sequence.len() as u32,
            };
            return (
                Some(same),
                if sequence.is_empty() {
                    vec![]
                } else {
                    vec![all]
                },
            );
        }
        let Some((previous, _)) = self.recent.first() else {
            return (None, insert_all(sequence));
        };
        let previous = *previous;
        let from_previous = self.delta(0, sequence);
        // A version that keeps little of the one before it may keep
        // more of the one before that.
        if inserted(&from_previous) * 4 > sequence.len() && self.recent.len() > 1 {
            let from_earlier = self.delta(1, sequence);
            if inserted(&from_earlier) < inserted(&from_previous) {
                return (Some(self.recent[1].0), from_earlier);
            }
        }
        (Some(previous), from_previous)
    }

    /// The instructions that make `sequence` from the `nth` most recent
    /// version.
    fn delta(&mut self, nth: usize, sequence: &[u32]) -> Vec<Op> {
        let base: Vec<Token> = self.recent[nth].1.iter().map(|&c| Token(c)).collect();
        let new: Vec<Token> = sequence.iter().map(|&c| Token(c)).collect();
        self.diff
            .compute_with(Algorithm::Histogram, &base, &new, self.chunks.len() as u32);
        let mut ops = Vec::new();
        let (mut old, mut at) = (0, 0);
        let copy = |ops: &mut Vec<Op>, start: u32, len: u32| {
            if len > 0 {
                ops.push(Op::Copy { start, len });
            }
        };
        for hunk in self.diff.hunks() {
            copy(&mut ops, old, hunk.before.start - old);
            if !hunk.after.is_empty() {
                let added = &sequence[hunk.after.start as usize..hunk.after.end as usize];
                ops.push(Op::Insert(added.to_vec()));
            }
            (old, at) = (hunk.before.end, hunk.after.end);
        }
        copy(&mut ops, old, sequence.len() as u32 - at);
        ops
    }

    /// Records what is derived about a version just added.
    fn admit(&mut self, index: u32, sequence: Vec<u32>, text_len: u64) {
        let depth = match self.entries[index as usize].base {
            Some(base) => self.depths[base as usize] + 1,
            None => 0,
        };
        self.depths.push(depth);
        self.lens.push(sequence.len() as u32);
        self.text_lens.push(text_len);
        self.by_hash.entry(hash(&sequence)).or_insert(index);
        if depth % CHECKPOINT == 0 {
            self.checkpoints.insert(index, sequence.clone());
        }
        self.recent.insert(0, (index, sequence));
        self.recent.truncate(2);
    }

    /// A version's chunk sequence: found whole, or made by following its
    /// bases back to one that is and carrying out the instructions since.
    fn sequence(&self, index: u32) -> Vec<u32> {
        let mut steps = Vec::new();
        let mut at = index;
        let mut sequence = loop {
            if let Some(whole) = self.whole(at, &self.recent) {
                break whole.to_vec();
            }
            steps.push(at);
            match self.entries[at as usize].base {
                Some(base) => at = base,
                None => break Vec::new(),
            }
        };
        for step in steps.into_iter().rev() {
            sequence = apply(&sequence, &self.entries[step as usize].ops);
        }
        sequence
    }

    /// The sequence of `index`, given those of the versions just before
    /// it in `recent`.
    fn next_sequence(&self, index: u32, recent: &[(u32, Vec<u32>)]) -> Vec<u32> {
        let entry = &self.entries[index as usize];
        match entry.base {
            None => apply(&[], &entry.ops),
            Some(base) => match self.whole(base, recent) {
                Some(whole) => apply(whole, &entry.ops),
                None => apply(&self.sequence(base), &entry.ops),
            },
        }
    }

    fn whole<'a>(&'a self, index: u32, recent: &'a [(u32, Vec<u32>)]) -> Option<&'a [u32]> {
        let recent = recent.iter().find(|(version, _)| *version == index);
        recent
            .map(|(_, sequence)| sequence.as_slice())
            .or_else(|| self.checkpoints.get(&index).map(Vec::as_slice))
    }

    /// The archive as the bytes of a file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut body = Vec::new();
        put_str(&mut body, &self.title);
        put(&mut body, self.mode as u64);
        put(&mut body, self.authors.len() as u64);
        for author in &self.authors {
            put_str(&mut body, &author.name);
            put(&mut body, u64::from(author.anonymous));
        }
        put(&mut body, self.chunks.len() as u64);
        for chunk in &self.chunks {
            put_str(&mut body, chunk);
        }
        put(&mut body, self.entries.len() as u64);
        let mut last_time = 0;
        for (index, entry) in self.entries.iter().enumerate() {
            put_str(&mut body, &entry.id);
            put(&mut body, u64::from(entry.author));
            // Times as differences, which are small; zigzag, as they can
            // run backwards.
            let step = entry.timestamp.wrapping_sub(last_time);
            put(&mut body, ((step << 1) ^ (step >> 63)) as u64);
            last_time = entry.timestamp;
            put_str(&mut body, &entry.comment);
            // The base as a distance back: 0 for none.
            put(
                &mut body,
                entry.base.map_or(0, |base| index as u64 - u64::from(base)),
            );
            put(&mut body, entry.ops.len() as u64);
            // A copy's start as the distance past the end of the copy
            // before it, which is small and, the copies being in order
            // from a diff, almost never negative; zigzag for when it is.
            let mut copied_to = 0i64;
            for op in &entry.ops {
                match op {
                    Op::Copy { start, len } => {
                        put(&mut body, u64::from(*len) << 1);
                        let skip = i64::from(*start) - copied_to;
                        put(&mut body, ((skip << 1) ^ (skip >> 63)) as u64);
                        copied_to = i64::from(*start) + i64::from(*len);
                    }
                    Op::Insert(chunks) => {
                        put(&mut body, (chunks.len() as u64) << 1 | 1);
                        for &chunk in chunks {
                            put(&mut body, u64::from(chunk));
                        }
                    }
                }
            }
        }

        let mut bytes = MAGIC.to_vec();
        let mut deflater = DeflateEncoder::new(&mut bytes, Compression::best());
        deflater.write_all(&body).expect("writing to memory");
        deflater.finish().expect("writing to memory");
        bytes
    }

    /// Whether `bytes` begin as an archive does.
    pub fn is_archive(bytes: &[u8]) -> bool {
        bytes.starts_with(MAGIC)
    }

    /// Reads an archive from the bytes of a file, checking that every
    /// instruction in it can be carried out.
    pub fn from_bytes(bytes: &[u8]) -> Result<Archive, String> {
        let deflated = bytes
            .strip_prefix(MAGIC)
            .ok_or("not a history flow archive")?;
        let mut body = Vec::new();
        DeflateDecoder::new(deflated)
            .take(MAX_INFLATED)
            .read_to_end(&mut body)
            .map_err(|e| format!("damaged archive: {e}"))?;
        let mut input = Input { bytes: &body };

        let mut archive = Archive::new(input.string()?, Mode::default());
        archive.mode = match input.number()? {
            0 => Mode::Lines,
            1 => Mode::Prose,
            2 => Mode::Sentences,
            other => return Err(format!("damaged archive: unknown mode {other}")),
        };
        for _ in 0..input.number()? {
            let author = Author {
                name: input.string()?,
                anonymous: input.number()? != 0,
            };
            archive
                .author_ids
                .insert(author.clone(), archive.authors.len() as u32);
            archive.authors.push(author);
        }
        for _ in 0..input.number()? {
            let chunk: Box<str> = input.string()?.into();
            archive
                .chunk_ids
                .insert(chunk.clone(), archive.chunks.len() as u32);
            archive.chunks.push(chunk);
        }

        let damaged = |what: &str| format!("damaged archive: {what}");
        let mut last_time = 0i64;
        for index in 0..input.number()? {
            let index = u32::try_from(index).map_err(|_| damaged("too many versions"))?;
            let id = input.string()?;
            let author = input.small()?;
            if author as usize >= archive.authors.len() {
                return Err(damaged("no such author"));
            }
            let step = input.number()?;
            last_time = last_time.wrapping_add((step >> 1) as i64 ^ -((step & 1) as i64));
            let comment = input.string()?;
            let base = match input.small()? {
                0 => None,
                back if back <= index => Some(index - back),
                _ => return Err(damaged("a version based on a later one")),
            };
            let base_len = base.map_or(0, |base| archive.lens[base as usize]);

            let mut ops = Vec::new();
            let mut len = 0usize;
            let mut copied_to = 0i64;
            for _ in 0..input.number()? {
                let head = input.number()?;
                let count =
                    u32::try_from(head >> 1).map_err(|_| damaged("an oversized instruction"))?;
                len += count as usize;
                if len > MAX_CHUNKS {
                    return Err(damaged("an oversized version"));
                }
                if head & 1 == 0 {
                    let skip = input.number()?;
                    let start = copied_to + ((skip >> 1) as i64 ^ -((skip & 1) as i64));
                    let end = start + i64::from(count);
                    if start < 0 || end > i64::from(base_len) {
                        return Err(damaged("a copy from outside its base"));
                    }
                    copied_to = end;
                    ops.push(Op::Copy {
                        start: start as u32,
                        len: count,
                    });
                } else {
                    let mut chunks = Vec::new();
                    for _ in 0..count {
                        let chunk = input.small()?;
                        if chunk as usize >= archive.chunks.len() {
                            return Err(damaged("no such chunk"));
                        }
                        chunks.push(chunk);
                    }
                    ops.push(Op::Insert(chunks));
                }
            }
            archive.entries.push(Entry {
                id,
                author,
                timestamp: last_time,
                comment,
                base,
                ops,
            });
            let sequence = archive.next_sequence(index, &archive.recent);
            let text_len = sequence
                .iter()
                .map(|&c| archive.chunks[c as usize].len() as u64)
                .sum();
            archive.admit(index, sequence, text_len);
        }
        if !input.bytes.is_empty() {
            return Err(damaged("bytes left over"));
        }
        Ok(archive)
    }
}

/// Splits text into the pieces an archive stores once each: a run of
/// letters, digits and underscores; a run of spaces and tabs; or any
/// other single character. Every byte of the text is in exactly one.
fn chunks(text: &str) -> impl Iterator<Item = &str> {
    #[derive(PartialEq, Clone, Copy)]
    enum Kind {
        Word,
        Blank,
        Other,
    }
    let kind = |c: char| match c {
        c if c.is_alphanumeric() || c == '_' => Kind::Word,
        ' ' | '\t' => Kind::Blank,
        _ => Kind::Other,
    };
    let mut rest = text;
    std::iter::from_fn(move || {
        let mut chars = rest.char_indices();
        let (_, first) = chars.next()?;
        let first = kind(first);
        let end = match first {
            Kind::Other => chars.next().map(|(at, _)| at),
            _ => chars.find(|&(_, c)| kind(c) != first).map(|(at, _)| at),
        };
        let (chunk, after) = rest.split_at(end.unwrap_or(rest.len()));
        rest = after;
        Some(chunk)
    })
}

fn apply(base: &[u32], ops: &[Op]) -> Vec<u32> {
    let mut sequence = Vec::new();
    for op in ops {
        match op {
            Op::Copy { start, len } => {
                sequence.extend_from_slice(&base[*start as usize..(*start + *len) as usize]);
            }
            Op::Insert(chunks) => sequence.extend_from_slice(chunks),
        }
    }
    sequence
}

fn insert_all(sequence: &[u32]) -> Vec<Op> {
    if sequence.is_empty() {
        vec![]
    } else {
        vec![Op::Insert(sequence.to_vec())]
    }
}

/// How many chunks a list of instructions spells out.
fn inserted(ops: &[Op]) -> usize {
    ops.iter()
        .map(|op| match op {
            Op::Insert(chunks) => chunks.len(),
            Op::Copy { .. } => 0,
        })
        .sum()
}

fn hash(sequence: &[u32]) -> u64 {
    let mut hasher = DefaultHasher::new();
    sequence.hash(&mut hasher);
    hasher.finish()
}

/// Writes a number in as few bytes as it needs: seven bits to a byte,
/// low bits first, the top bit set on all but the last.
fn put(out: &mut Vec<u8>, mut number: u64) {
    while number >= 0x80 {
        out.push(number as u8 | 0x80);
        number >>= 7;
    }
    out.push(number as u8);
}

fn put_str(out: &mut Vec<u8>, text: &str) {
    put(out, text.len() as u64);
    out.extend_from_slice(text.as_bytes());
}

struct Input<'a> {
    bytes: &'a [u8],
}

impl Input<'_> {
    fn number(&mut self) -> Result<u64, String> {
        let mut number = 0u64;
        for shift in (0..64).step_by(7) {
            let (&byte, rest) = self
                .bytes
                .split_first()
                .ok_or("damaged archive: cut short")?;
            self.bytes = rest;
            number |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Ok(number);
            }
        }
        Err("damaged archive: an overlong number".into())
    }

    fn small(&mut self) -> Result<u32, String> {
        u32::try_from(self.number()?).map_err(|_| "damaged archive: a number out of range".into())
    }

    fn string(&mut self) -> Result<String, String> {
        let len = self.number()?;
        if len > self.bytes.len() as u64 {
            return Err("damaged archive: cut short".into());
        }
        let (text, rest) = self.bytes.split_at(len as usize);
        self.bytes = rest;
        String::from_utf8(text.to_vec())
            .map_err(|_| "damaged archive: text that is not UTF-8".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(index: usize, text: &str) -> SourceVersion {
        SourceVersion {
            id: format!("r{index}"),
            author: ["ada", "203.0.113.7", "brooke"][index % 3].into(),
            anonymous: index % 3 == 1,
            timestamp: 1_000_000 + [0, 50, -20, 86_400][index % 4] * index as i64,
            comment: format!("edit {index}\nsecond line"),
            text: text.into(),
        }
    }

    fn archive(texts: &[&str]) -> Archive {
        let mut archive = Archive::new("A title", Mode::Prose);
        for (index, text) in texts.iter().enumerate() {
            archive.push(&version(index, text));
        }
        archive
    }

    /// Everything about an archive that a reader can see.
    fn contents(archive: &Archive) -> (String, Mode, Vec<SourceVersion>) {
        let versions: Vec<_> = archive.versions().collect();
        let one_by_one: Vec<_> = (0..archive.len()).map(|i| archive.version(i)).collect();
        assert_eq!(versions, one_by_one);
        (archive.title.clone(), archive.mode, versions)
    }

    #[test]
    fn chunks_partition_any_text() {
        let text = "fn  a_1(x: &str) {\r\n\tx.len()  // naïve—ok\n}\n\n";
        let pieces: Vec<_> = chunks(text).collect();
        assert_eq!(pieces.concat(), text);
        assert_eq!(
            pieces[..12],
            [
                "fn", "  ", "a_1", "(", "x", ":", " ", "&", "str", ")", " ", "{"
            ]
        );
        assert_eq!(chunks("").count(), 0);
    }

    #[test]
    fn every_version_comes_back_exactly() {
        let texts = [
            "",
            "One two three.\n",
            "One two  three.\nFour five.\n",
            "\tTabs\tand   spaces \r\n and a naïve café — 東京.",
            "One two three.\n",
            "",
            " ",
        ];
        let stored = archive(&texts);
        let (title, mode, versions) = contents(&stored);
        assert_eq!((title.as_str(), mode), ("A title", Mode::Prose));
        for (index, text) in texts.iter().enumerate() {
            assert_eq!(versions[index], version(index, text));
            assert_eq!(stored.text(index), *text);
        }
        assert_eq!(
            stored.text_bytes(),
            texts.iter().map(|t| t.len() as u64).sum::<u64>()
        );
    }

    #[test]
    fn a_file_round_trips_and_can_be_added_to() {
        let texts = ["Alpha beta.", "Alpha beta gamma.", "x", "Alpha beta gamma."];
        let stored = archive(&texts);
        let bytes = stored.to_bytes();
        assert!(Archive::is_archive(&bytes));
        let mut read = Archive::from_bytes(&bytes).unwrap();
        assert_eq!(contents(&read), contents(&stored));
        // Reading and writing again gives the same file.
        assert_eq!(read.to_bytes(), bytes);

        let mut longer = stored;
        for archive in [&mut read, &mut longer] {
            archive.push(&version(4, "Alpha beta."));
            archive.push(&version(5, "Alpha delta gamma."));
        }
        assert_eq!(contents(&read), contents(&longer));
        assert_eq!(read.to_bytes(), longer.to_bytes());
    }

    #[test]
    fn either_kind_of_file_is_read_and_a_bundle_round_trips() {
        let stored = archive(&["Alpha.", "Alpha beta."]);
        let bundle = stored.to_bundle();
        assert_eq!(bundle.versions[1], version(1, "Alpha beta."));
        let json = serde_json::to_vec(&bundle).unwrap();
        for bytes in [json, stored.to_bytes()] {
            assert_eq!(contents(&Archive::read(&bytes).unwrap()), contents(&stored));
        }
        assert!(
            Archive::read(b"nonsense")
                .unwrap_err()
                .starts_with("neither")
        );
        let other = br#"{"format":"x/9","title":"","mode":"lines","versions":[]}"#;
        assert_eq!(
            Archive::read(other).unwrap_err(),
            "unsupported bundle format \"x/9\""
        );
    }

    #[test]
    fn restored_and_alternating_versions_cost_almost_nothing() {
        let page: String = (0..2000).map(|i| format!("word{i} ")).collect();
        let other: String = (0..2000).map(|i| format!("else{i} ")).collect();
        let once = archive(&[&page, &other]).to_bytes().len();
        // Vandalised and restored five times, then an edit war between
        // the two pages, each side changing a little every round.
        let mut texts: Vec<String> = Vec::new();
        for _ in 0..5 {
            texts.push(page.clone());
            texts.push("RUBBISH".into());
        }
        for round in 0..5 {
            texts.push(format!("{page}mine{round}"));
            texts.push(format!("{other}theirs{round}"));
        }
        let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
        let stored = archive(&texts);
        let many = stored.to_bytes().len();
        assert!(many < once + 1200, "{many} against {once}");
        assert!(stored.text_bytes() > 200_000);
        for (index, text) in texts.iter().enumerate() {
            assert_eq!(stored.text(index), *text);
        }
    }

    #[test]
    fn long_and_random_histories_come_back_exactly() {
        let words = ["a", "b", "c", "d", ".", "\n", "  ", "e_f", "é"];
        let mut seed = 0x1234_5678_9abc_def1_u64;
        let mut random = move |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let mut doc: Vec<&str> = Vec::new();
        let mut texts: Vec<String> = Vec::new();
        // Well past several checkpoints.
        for _ in 0..(CHECKPOINT as usize * 5) {
            match random(8) {
                0 if !texts.is_empty() => {
                    texts.push(texts[random(texts.len())].clone());
                    continue;
                }
                1 if !doc.is_empty() => {
                    let start = random(doc.len());
                    doc.drain(start..(start + random(6)).min(doc.len()));
                }
                2 => doc.clear(),
                _ => {
                    let at = random(doc.len() + 1);
                    let added: Vec<_> = (0..1 + random(5))
                        .map(|_| words[random(words.len())])
                        .collect();
                    doc.splice(at..at, added);
                }
            }
            texts.push(doc.join(" "));
        }
        let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
        let stored = archive(&texts);
        let read = Archive::from_bytes(&stored.to_bytes()).unwrap();
        for archive in [&stored, &read] {
            for (index, text) in texts.iter().enumerate().rev() {
                assert_eq!(archive.text(index), *text, "version {index}");
            }
            assert!(archive.versions().map(|v| v.text).eq(texts.iter().copied()));
        }
    }

    #[test]
    fn a_long_chain_of_small_edits_is_read_from_checkpoints() {
        let mut text = String::new();
        let texts: Vec<String> = (0..CHECKPOINT as usize * 4 + 7)
            .map(|i| {
                text.push_str(&format!("w{i} "));
                text.clone()
            })
            .collect();
        let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
        let stored = archive(&texts);
        let read = Archive::from_bytes(&stored.to_bytes()).unwrap();
        for archive in [&stored, &read] {
            // One whole sequence per CHECKPOINT steps, not one per version.
            assert_eq!(archive.checkpoints.len(), 5);
            for index in [0, 1, 63, 64, 65, 200, texts.len() - 1] {
                assert_eq!(archive.text(index), texts[index]);
            }
        }
    }

    #[test]
    fn damaged_files_are_refused_not_trusted() {
        let bytes = archive(&["Alpha beta.", "Alpha gamma.", "Alpha beta."]).to_bytes();
        assert_eq!(
            Archive::from_bytes(b"{}").unwrap_err(),
            "not a history flow archive"
        );
        // Cut short anywhere.
        for len in MAGIC.len()..bytes.len() {
            assert!(Archive::from_bytes(&bytes[..len]).is_err(), "cut to {len}");
        }
        // Well-formed deflate around instructions that cannot be carried
        // out: each must be caught, whatever it is.
        let wrap = |body: &[u8]| {
            let mut bytes = MAGIC.to_vec();
            let mut deflater = DeflateEncoder::new(&mut bytes, Compression::fast());
            deflater.write_all(body).unwrap();
            deflater.finish().unwrap();
            bytes
        };
        // title "", mode 0, one author "a", one chunk "x", then versions.
        let head = [0, 0, 1, 1, b'a', 0, 1, 1, b'x'];
        let version = |rest: &[u8]| {
            // one version: id "", author 0, time 0, comment "", then rest.
            let mut body = head.to_vec();
            body.extend([1, 0, 0, 0, 0]);
            body.extend(rest);
            wrap(&body)
        };
        // Sound: no base, one instruction inserting chunk 0.
        assert_eq!(
            Archive::from_bytes(&version(&[0, 1, 3, 0]))
                .unwrap()
                .text(0),
            "x"
        );
        let refused = [
            (&[1, 0][..], "a version based on a later one"),
            (&[0, 1, 3, 7][..], "no such chunk"),
            (&[0, 1, 2, 0][..], "a copy from outside its base"),
            (&[0, 1, 3, 0, 9][..], "bytes left over"),
            (
                &[0, 1, 0xff, 0xff, 0xff, 0xff, 0x0f, 0][..],
                "an oversized version",
            ),
        ];
        for (rest, why) in refused {
            let error = Archive::from_bytes(&version(rest)).unwrap_err();
            assert_eq!(error, format!("damaged archive: {why}"));
        }
        let mut no_author = head.to_vec();
        no_author.extend([1, 0, 5, 0, 0, 0, 0]);
        assert_eq!(
            Archive::from_bytes(&wrap(&no_author)).unwrap_err(),
            "damaged archive: no such author"
        );
    }
}
