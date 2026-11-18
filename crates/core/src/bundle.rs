//! The interchange file between data sources and the page.

use serde::{Deserialize, Serialize};

use crate::attribute::Config;
use crate::model::SourceVersion;

/// How a document should be compared, as a name a file or URL can carry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// [`Config::source_lines`].
    #[default]
    Lines,
    /// [`Config::prose`].
    Prose,
    /// [`Config::sentences_2004`].
    Sentences,
}

impl Mode {
    pub fn config(self) -> Config {
        match self {
            Mode::Lines => Config::source_lines(),
            Mode::Prose => Config::prose(),
            Mode::Sentences => Config::sentences_2004(),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Mode::Lines => "lines",
            Mode::Prose => "prose",
            Mode::Sentences => "sentences",
        }
    }

    pub fn from_name(name: &str) -> Option<Mode> {
        [Mode::Lines, Mode::Prose, Mode::Sentences]
            .into_iter()
            .find(|mode| mode.name() == name)
    }
}

/// A document's whole history as a data source hands it over: JSON, one
/// file. The versions carry their full text, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bundle {
    /// Always [`Bundle::FORMAT`]; lets a reader reject some other JSON.
    pub format: String,
    pub title: String,
    /// The comparison the source suggests; the viewer may override it.
    pub mode: Mode,
    pub versions: Vec<SourceVersion>,
}

impl Bundle {
    pub const FORMAT: &'static str = "historyflow-bundle/1";

    pub fn new(title: impl Into<String>, mode: Mode, versions: Vec<SourceVersion>) -> Bundle {
        Bundle {
            format: Bundle::FORMAT.into(),
            title: title.into(),
            mode,
            versions,
        }
    }
}
