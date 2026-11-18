//! The browser-facing half: wasm-bindgen exports and the WebGL2 renderer.
//! The page (`web/`) owns all UI; this crate owns the canvas.

mod renderer;

use historyflow_core::{Bundle, History, HistoryBuilder, Mode, Spacing, author_colors, layout};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use renderer::{Coloring, Renderer};

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
    history: History,
    spacing: Spacing,
    coloring: Coloring,
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
            history: History::default(),
            spacing: Spacing::default(),
            coloring: Coloring::default(),
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
        self.upload()?;

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

    /// Draws the plot. `pixel_ratio` is CSS pixels to canvas pixels.
    pub fn draw(&self, pixel_ratio: f32) {
        let versions = self.history.versions().len();
        self.renderer.draw(self.coloring, versions, pixel_ratio);
    }

    fn upload(&mut self) -> Result<(), JsValue> {
        let layout = layout(&self.history, self.spacing);
        let colors = author_colors(self.history.authors());
        self.renderer.upload(&layout, &colors)
    }
}
