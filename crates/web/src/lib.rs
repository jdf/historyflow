//! The browser-facing half: wasm-bindgen exports and the WebGL2 renderer.
//! The page (`web/`) owns all UI; this crate owns the canvas.

mod renderer;

use historyflow_core::{
    Bundle, History, HistoryBuilder, Mode, Spacing, VersionIndex, author_colors, layout, passages,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use renderer::{Coloring, MARGIN, Overlay, Renderer};

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

/// The core crate's version.
#[wasm_bindgen]
pub fn version() -> String {
    historyflow_core::version().to_string()
}

/// One canvas showing one document's history.
#[wasm_bindgen]
pub struct App {
    renderer: Renderer,
    /// The loaded bundle; its versions' text is what the text panel shows.
    bundle: Bundle,
    mode: Mode,
    history: History,
    spacing: Spacing,
    coloring: Coloring,
    /// The x of each version, from the current layout.
    xs: Vec<f32>,
    cursor: Option<Cursor>,
}

/// The crosshair: a version, and a position in it in the layout's y.
#[derive(Clone, Copy)]
struct Cursor {
    version: VersionIndex,
    y: f32,
}

/// One version for the text panel, as JSON.
#[derive(Serialize)]
struct Detail<'a> {
    index: VersionIndex,
    id: &'a str,
    /// Index into the summary's authors.
    author: u32,
    timestamp: i64,
    comment: &'a str,
    /// Text, author, origin version, top y, bottom y. The texts joined
    /// are the version's whole text.
    passages: Vec<(&'a str, u32, VersionIndex, f32, f32)>,
}

/// What the page needs to know about a loaded history, as JSON.
#[derive(Serialize)]
struct Summary<'a> {
    title: &'a str,
    mode: &'a str,
    versions: usize,
    tokens: usize,
    first_timestamp: Option<i64>,
    last_timestamp: Option<i64>,
    authors: Vec<AuthorSummary<'a>>,
}

#[derive(Serialize)]
struct AuthorSummary<'a> {
    name: &'a str,
    anonymous: bool,
    /// `#rrggbb`.
    color: String,
    /// How many versions the author made.
    versions: usize,
}

#[wasm_bindgen]
impl App {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: &HtmlCanvasElement) -> Result<App, JsValue> {
        Ok(App {
            renderer: Renderer::new(canvas)?,
            bundle: Bundle::new("", Mode::default(), Vec::new()),
            mode: Mode::default(),
            history: History::default(),
            spacing: Spacing::default(),
            coloring: Coloring::default(),
            xs: Vec::new(),
            cursor: None,
        })
    }

    /// Loads a bundle (the JSON a data source wrote) and returns a JSON
    /// summary of it. `mode` is `lines`, `prose` or `sentences`, or empty
    /// to use the one the bundle suggests.
    pub fn load(&mut self, bundle: &str, mode: &str) -> Result<String, JsValue> {
        let bundle: Bundle =
            serde_json::from_str(bundle).map_err(|e| format!("not a history flow bundle: {e}"))?;
        if bundle.format != Bundle::FORMAT {
            return Err(format!("unsupported bundle format {:?}", bundle.format).into());
        }
        let mode = match mode {
            "" => bundle.mode,
            name => Mode::from_name(name).ok_or_else(|| format!("unknown mode {name:?}"))?,
        };
        let mut builder = HistoryBuilder::new(mode.config());
        for version in &bundle.versions {
            builder.push(version);
        }
        self.history = builder.finish();
        self.mode = mode;
        self.bundle = bundle;
        self.cursor = None;
        self.upload()?;
        let bundle = &self.bundle;

        let history = &self.history;
        let colors = author_colors(history.authors());
        let byte = |v: f32| (v * 255.0).round() as u8;
        let authors = history
            .authors()
            .iter()
            .zip(&colors)
            .enumerate()
            .map(|(id, (author, c))| AuthorSummary {
                name: &author.name,
                anonymous: author.anonymous,
                color: format!("#{:02x}{:02x}{:02x}", byte(c[0]), byte(c[1]), byte(c[2])),
                versions: history
                    .versions()
                    .iter()
                    .filter(|v| v.author.0 as usize == id)
                    .count(),
            })
            .collect();
        let summary = Summary {
            title: &bundle.title,
            mode: mode.name(),
            versions: history.versions().len(),
            tokens: history.token_count(),
            first_timestamp: history.versions().first().map(|v| v.timestamp),
            last_timestamp: history.versions().last().map(|v| v.timestamp),
            authors,
        };
        Ok(serde_json::to_string(&summary).expect("serializable"))
    }

    /// `version` (evenly spaced) or `date`.
    pub fn set_spacing(&mut self, spacing: &str) -> Result<(), JsValue> {
        self.spacing = match spacing {
            "version" => Spacing::Version,
            "date" => Spacing::Date,
            other => return Err(format!("unknown spacing {other:?}").into()),
        };
        self.upload()
    }

    /// `author` (hue by author, darkening with age) or `age` (greys).
    pub fn set_coloring(&mut self, coloring: &str) -> Result<(), JsValue> {
        self.coloring = match coloring {
            "author" => Coloring::Author,
            "age" => Coloring::Age,
            other => return Err(format!("unknown coloring {other:?}").into()),
        };
        Ok(())
    }

    /// The version and position under a point of the canvas, given in
    /// CSS pixels along with the canvas's CSS size: `[version, y]`, or
    /// empty if nothing is loaded. The version is the nearest one; y is
    /// in the layout's units and may lie beyond the version's end.
    pub fn pick(&self, x: f32, y: f32, width: f32, height: f32) -> Vec<f32> {
        if self.xs.is_empty() {
            return Vec::new();
        }
        let x = (x - MARGIN) / (width - 2.0 * MARGIN).max(1.0);
        let y = (y - MARGIN) / (height - 2.0 * MARGIN).max(1.0);
        let after = self.xs.partition_point(|&version_x| version_x < x);
        let nearest = match (after.checked_sub(1), self.xs.get(after)) {
            (Some(before), Some(&next)) if x - self.xs[before] <= next - x => before,
            (Some(before), None) => before,
            _ => after,
        };
        vec![nearest as f32, y]
    }

    /// Moves the crosshair, keeping it within the version's text, and
    /// returns the y it ended up at.
    pub fn set_cursor(&mut self, version: u32, y: f32) -> Result<f32, JsValue> {
        let found = self.history.versions().get(version as usize);
        let found = found.ok_or_else(|| format!("no version {version}"))?;
        let end = (found.weight() as f64 / self.history.tallest() as f64) as f32;
        let y = y.clamp(0.0, end);
        self.cursor = Some(Cursor { version, y });
        Ok(y)
    }

    /// A version's metadata and its text divided by origin, as JSON.
    pub fn version_detail(&self, version: u32) -> Result<String, JsValue> {
        let found = self.history.versions().get(version as usize);
        let found = found.ok_or_else(|| format!("no version {version}"))?;
        let text = self.bundle.versions[version as usize].text.as_str();
        let segmenter = self.mode.config().segmenter;
        let detail = Detail {
            index: version,
            id: &found.id,
            author: found.author.0,
            timestamp: found.timestamp,
            comment: &found.comment,
            passages: passages(&self.history, version, text, &*segmenter)
                .into_iter()
                .map(|p| {
                    let author = self.history.versions()[p.origin as usize].author.0;
                    (&text[p.text], author, p.origin, p.y[0], p.y[1])
                })
                .collect(),
        };
        Ok(serde_json::to_string(&detail).expect("serializable"))
    }

    /// Draws the plot. `pixel_ratio` is CSS pixels to canvas pixels.
    pub fn draw(&self, pixel_ratio: f32) {
        let versions = self.history.versions();
        let overlay = self.cursor.map(|cursor| {
            // Mark every version by the author of the selected one.
            let author = versions[cursor.version as usize].author;
            Overlay {
                x: self.xs[cursor.version as usize],
                y: cursor.y,
                marks: (versions.iter().zip(&self.xs))
                    .filter(|(version, _)| version.author == author)
                    .map(|(_, &x)| x)
                    .collect(),
            }
        });
        self.renderer
            .draw(self.coloring, versions.len(), pixel_ratio, overlay.as_ref());
    }

    fn upload(&mut self) -> Result<(), JsValue> {
        let layout = layout(&self.history, self.spacing);
        let colors = author_colors(self.history.authors());
        self.renderer.upload(&layout, &colors)?;
        self.xs = layout.xs;
        Ok(())
    }
}
