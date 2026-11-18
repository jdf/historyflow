//! The browser-facing half: wasm-bindgen exports and the WebGL2 renderer.
//! The page (`web/`) owns all UI; this crate owns the canvas.

mod renderer;

use historyflow_core::{
    Bundle, History, HistoryBuilder, Mode, SourceVersion, Spacing, VersionIndex, View, YScale,
    author_colors, layout, passages, wikipedia,
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
    /// Holds the history, and can add versions to it as a source that
    /// arrives in batches delivers them.
    builder: HistoryBuilder,
    spacing: Spacing,
    coloring: Coloring,
    /// The x of each version, from the current layout.
    xs: Vec<f32>,
    /// Each version's length, in the layout's y.
    extents: Vec<f32>,
    /// The part of the history on screen.
    view: View,
    /// Whether the plot's height fits the versions on screen, not the
    /// longest of the whole history.
    fit_view: bool,
    /// Whether length is drawn on a logarithmic scale.
    log: bool,
    /// The vertical scale last drawn, which eases toward the one the
    /// settings and the view call for; `None` until something is drawn,
    /// so that a newly loaded history starts at rest.
    shown: Option<YScale>,
    /// When it was drawn, in milliseconds, and whether it was still
    /// easing then.
    drawn_at: f64,
    easing: bool,
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
            builder: HistoryBuilder::new(Mode::default().config()),
            spacing: Spacing::default(),
            coloring: Coloring::default(),
            xs: Vec::new(),
            extents: Vec::new(),
            view: View::FULL,
            fit_view: false,
            log: false,
            shown: None,
            drawn_at: 0.0,
            easing: false,
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
            name => parse_mode(name)?,
        };
        self.bundle = bundle;
        self.rebuild(mode)
    }

    /// Compares the loaded versions afresh in another mode, and returns
    /// the new summary.
    pub fn set_mode(&mut self, mode: &str) -> Result<String, JsValue> {
        self.rebuild(parse_mode(mode)?)
    }

    /// Starts an empty history, to be filled by [`App::add_wikipedia`].
    pub fn begin(&mut self, title: &str, mode: &str) -> Result<(), JsValue> {
        self.bundle = Bundle::new(title, parse_mode(mode)?, Vec::new());
        self.rebuild(self.bundle.mode).map(drop)
    }

    /// Adds the revisions in one response of the MediaWiki API to the
    /// history, and returns what to pass to [`wikipedia_request`] for
    /// the next batch, or nothing if this was the last.
    pub fn add_wikipedia(&mut self, response: &str) -> Result<Option<String>, JsValue> {
        let batch = wikipedia::parse(response)?;
        self.bundle.title = batch.title;
        self.add(batch.versions)?;
        Ok(batch.next)
    }

    /// The loaded history as a bundle, to save.
    pub fn bundle(&self) -> String {
        serde_json::to_string(&self.bundle).expect("serializable")
    }

    /// What the page needs to know about the loaded history, as JSON.
    pub fn summary(&self) -> String {
        let history = self.history();
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
            title: &self.bundle.title,
            mode: self.mode.name(),
            versions: history.versions().len(),
            tokens: history.token_count(),
            first_timestamp: history.versions().first().map(|v| v.timestamp),
            last_timestamp: history.versions().last().map(|v| v.timestamp),
            authors,
        };
        serde_json::to_string(&summary).expect("serializable")
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

    /// What the plot's height stands for: `all`, the longest version in
    /// the history, or `view`, the longest of those on screen, so that
    /// one enormous version squeezes the rest only while it is in view.
    pub fn set_height(&mut self, height: &str) -> Result<(), JsValue> {
        self.fit_view = match height {
            "all" => false,
            "view" => true,
            other => return Err(format!("unknown height {other:?}").into()),
        };
        Ok(())
    }

    /// `linear`, or `log` to squeeze long versions progressively.
    pub fn set_scale(&mut self, scale: &str) -> Result<(), JsValue> {
        self.log = match scale {
            "linear" => false,
            "log" => true,
            other => return Err(format!("unknown scale {other:?}").into()),
        };
        Ok(())
    }

    /// The vertical scale in force, as `[top, log]`: see
    /// `historyflow_core::YScale`.
    pub fn y_scale_now(&self) -> Vec<f32> {
        let scale = self.y_scale();
        vec![scale.top, scale.log]
    }

    /// The version and position under a point of the canvas, given in
    /// CSS pixels along with the canvas's CSS size: `[version, y]`, or
    /// empty if nothing is loaded. The version is the nearest one; y is
    /// in the layout's units and may lie beyond the version's end.
    pub fn pick(&self, x: f32, y: f32, width: f32, height: f32) -> Vec<f32> {
        if self.xs.is_empty() {
            return Vec::new();
        }
        let x = self.view.at(across(x, width));
        let y = self.y_scale().at(across(y, height).max(0.0));
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
        let found = self.history().versions().get(version as usize);
        let found = found.ok_or_else(|| format!("no version {version}"))?;
        let end = (found.weight() as f64 / self.history().tallest() as f64) as f32;
        let y = y.clamp(0.0, end);
        self.cursor = Some(Cursor { version, y });
        Ok(y)
    }

    /// A version's metadata and its text divided by origin, as JSON.
    pub fn version_detail(&self, version: u32) -> Result<String, JsValue> {
        let found = self.history().versions().get(version as usize);
        let found = found.ok_or_else(|| format!("no version {version}"))?;
        let text = self.bundle.versions[version as usize].text.as_str();
        let segmenter = self.mode.config().segmenter;
        let detail = Detail {
            index: version,
            id: &found.id,
            author: found.author.0,
            timestamp: found.timestamp,
            comment: &found.comment,
            passages: passages(self.history(), version, text, &*segmenter)
                .into_iter()
                .map(|p| {
                    let author = self.history().versions()[p.origin as usize].author.0;
                    (&text[p.text], author, p.origin, p.y[0], p.y[1])
                })
                .collect(),
        };
        Ok(serde_json::to_string(&detail).expect("serializable"))
    }

    /// Zooms the time axis by `factor` (less than 1 zooms in) about the
    /// point `x` CSS pixels across a canvas `width` CSS pixels wide. Stops
    /// at the whole history one way and three revisions the other.
    pub fn zoom(&mut self, factor: f32, x: f32, width: f32) {
        let min_width = View::min_width(&self.xs);
        self.view = self.view.zoom(factor, across(x, width), min_width);
    }

    /// Slides the view `dx` CSS pixels later in the history (earlier if
    /// negative), for a canvas `width` CSS pixels wide.
    pub fn pan(&mut self, dx: f32, width: f32) {
        self.view = self.view.pan(dx / (width - 2.0 * MARGIN).max(1.0));
    }

    /// Slides the view, if need be, to bring a version into it.
    pub fn reveal(&mut self, version: u32) {
        if let Some(&x) = self.xs.get(version as usize) {
            self.view = self.view.reveal(x);
        }
    }

    /// The view as `[start, width]`, in fractions of the whole history.
    pub fn view(&self) -> Vec<f32> {
        vec![self.view.start, self.view.width]
    }

    pub fn set_view(&mut self, start: f32, width: f32) {
        self.view = View::new(start, width, View::min_width(&self.xs));
    }

    /// Draws the plot. `pixel_ratio` is CSS pixels to canvas pixels and
    /// `now` the time in milliseconds. Returns whether the vertical scale
    /// is still easing toward where it is going, in which case the page
    /// should draw again next frame.
    pub fn draw(&mut self, pixel_ratio: f32, now: f64) -> bool {
        let target = self.target_scale();
        let scale = match self.shown {
            // The time since the last frame counts only if that frame was
            // part of this movement.
            // …and a frame that took long counts as a short one, so that a
            // hitch slows the movement and does not skip it.
            Some(shown) if self.easing => {
                shown.eased_toward(target, ((now - self.drawn_at) as f32).min(50.0))
            }
            Some(shown) => shown.eased_toward(target, 0.0),
            None => target,
        };
        self.shown = Some(scale);
        self.drawn_at = now;
        self.easing = scale != target;

        let versions = self.history().versions();
        let overlay = self.cursor.map(|cursor| {
            // Mark every version by the author of the selected one.
            let author = versions[cursor.version as usize].author;
            Overlay {
                x: self.xs[cursor.version as usize],
                y: scale.down(cursor.y),
                marks: (versions.iter().zip(&self.xs))
                    .filter(|(version, _)| version.author == author)
                    .map(|(_, &x)| x)
                    .collect(),
            }
        });
        self.renderer.draw(
            self.coloring,
            versions.len(),
            self.view,
            scale,
            pixel_ratio,
            overlay.as_ref(),
        );
        self.easing
    }

    /// Ends any easing: the next draw is at rest. For a reader who has
    /// asked their system for less motion.
    pub fn settle(&mut self) {
        self.shown = None;
    }

    fn history(&self) -> &History {
        self.builder.history()
    }

    /// The vertical scale on screen.
    fn y_scale(&self) -> YScale {
        self.shown.unwrap_or_else(|| self.target_scale())
    }

    /// The vertical scale the settings and the view call for.
    fn target_scale(&self) -> YScale {
        let log = if self.log {
            YScale::log_for(&self.extents)
        } else {
            0.0
        };
        if !self.fit_view {
            return YScale { top: 1.0, log };
        }
        // The longest version in view; or, if the view falls between
        // versions, of the two it falls between.
        let (start, end) = (self.view.start, self.view.start + self.view.width);
        let first = self.xs.partition_point(|&x| x < start);
        let mut last = self.xs.partition_point(|&x| x <= end);
        let mut first = first.min(self.xs.len().saturating_sub(1));
        if first >= last {
            first = first.saturating_sub(1);
            last = (first + 2).min(self.xs.len());
        }
        let top = self.extents[first..last]
            .iter()
            .copied()
            .fold(0.0, f32::max);
        YScale {
            top: if top > 0.0 { top } else { 1.0 },
            log,
        }
    }

    /// Builds the history again from the bundle's versions.
    fn rebuild(&mut self, mode: Mode) -> Result<String, JsValue> {
        self.mode = mode;
        self.builder = HistoryBuilder::new(mode.config());
        for version in &self.bundle.versions {
            self.builder.push(version);
        }
        self.cursor = None;
        self.shown = None;
        self.upload()?;
        self.view = View::FULL;
        Ok(self.summary())
    }

    /// Appends versions to the history and the bundle.
    fn add(&mut self, versions: Vec<SourceVersion>) -> Result<(), JsValue> {
        for version in &versions {
            self.builder.push(version);
        }
        self.bundle.versions.extend(versions);
        self.upload()
    }

    fn upload(&mut self) -> Result<(), JsValue> {
        let layout = layout(self.history(), self.spacing);
        let colors = author_colors(self.history().authors());
        self.renderer.upload(&layout, &colors)?;
        let tallest = self.history().tallest() as f64;
        let extent = |weight: u64| (weight as f64 / tallest) as f32;
        self.extents = (self.history().versions().iter())
            .map(|version| extent(version.weight()))
            .collect();
        // Keep the same versions on screen when the spacing moves them,
        // or all of them if all of them were.
        if self.view == View::FULL {
            self.xs = layout.xs;
            return Ok(());
        }
        let (first, last) = (self.view.start, self.view.start + self.view.width);
        let (first, last) = (index_at(&self.xs, first), index_at(&self.xs, last));
        self.xs = layout.xs;
        let (start, end) = (x_at(&self.xs, first), x_at(&self.xs, last));
        self.view = View::new(start, end - start, View::min_width(&self.xs));
        Ok(())
    }
}

/// How far across the plot a canvas coordinate is: 0 at the plot's near
/// edge, 1 at its far edge. `size` is the canvas's, in the same units.
fn across(coordinate: f32, size: f32) -> f32 {
    (coordinate - MARGIN) / (size - 2.0 * MARGIN).max(1.0)
}

/// The version at `x`, as an index with a fraction for a point between
/// two versions.
fn index_at(xs: &[f32], x: f32) -> f32 {
    let after = xs.partition_point(|&version_x| version_x <= x);
    match (after.checked_sub(1), xs.get(after)) {
        (Some(before), Some(&next)) => before as f32 + (x - xs[before]) / (next - xs[before]),
        (Some(before), None) => before as f32,
        _ => 0.0,
    }
}

/// The inverse of [`index_at`].
fn x_at(xs: &[f32], index: f32) -> f32 {
    let Some(&last) = xs.last() else {
        return 0.0;
    };
    let before = (index.max(0.0) as usize).min(xs.len() - 1);
    let next = xs.get(before + 1).copied().unwrap_or(last);
    xs[before] + (index - before as f32).clamp(0.0, 1.0) * (next - xs[before])
}

fn parse_mode(name: &str) -> Result<Mode, JsValue> {
    Mode::from_name(name).ok_or_else(|| format!("unknown mode {name:?}").into())
}

/// The address to fetch for a batch of up to `limit` revisions of a
/// Wikipedia article, given as a title or as the article's address.
/// `next` is what [`App::add_wikipedia`] returned for the previous batch,
/// or empty for the first.
#[wasm_bindgen]
pub fn wikipedia_request(article: &str, next: &str, limit: u32) -> Result<String, JsValue> {
    let article = wikipedia::Article::parse(article)
        .ok_or_else(|| format!("not an article title or address: {article:?}"))?;
    Ok(article.request(next, limit))
}
