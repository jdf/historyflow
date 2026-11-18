//! The platform-independent model: document versions, the attribution of
//! their text to the versions that introduced it, and the links between
//! consecutive versions that the renderer draws as ribbons. No browser or
//! GL dependency, so everything here is tested natively with `cargo test`.
//!
//! The pipeline:
//!
//! 1. A data source yields [`SourceVersion`]s, oldest first: text plus
//!    who, when and why.
//! 2. A [`Segmenter`] splits each text into *atoms* (words, sentences or
//!    lines), grouped into units and blocks.
//! 3. A [`HistoryBuilder`] gives every atom a [`TokenId`]. An atom judged
//!    to be the same text as one in an earlier version reuses that
//!    token; anything else is a new token whose origin is the version
//!    being added. How "the same" is judged is the [`Matching`] strategy.
//! 4. The resulting [`History`] answers everything the visualization
//!    needs from token identity alone: a token's origin gives its author
//!    and age, and [`History::links`] gives the runs of tokens shared by
//!    two versions.
//!
//! 5. [`layout`] turns a history into the quads the renderer draws.
//!
//! A [`Bundle`] is the file a data source writes and the page reads: the
//! source versions plus a title and a suggested [`Mode`].
//!
//! See `docs/research/history-flow.md` §7 for why attribution and
//! alignment are both derived from token identity.

mod attribute;
mod bundle;
mod layout;
mod model;
mod segment;

pub use attribute::{Config, HistoryBuilder, Matching, Measure};
pub use bundle::{Bundle, Mode};
pub use layout::{Layout, Quad, Spacing, author_colors, layout};
pub use model::{
    Author, AuthorId, History, Link, Run, SourceVersion, TokenId, Version, VersionIndex,
};
pub use segment::{Lines, Prose, Segmentation, Segmenter, Sentences};

/// The crate version, surfaced in the page so a build can be identified.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
