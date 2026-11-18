# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What this is

A reimplementation of Viégas and Wattenberg's _history flow_ visualization: Rust
→ WebAssembly → WebGL2, with a vanilla HTML/JS page. Research on the original is
in `docs/research/history-flow.md`; consult it before making design decisions
about the visual encoding or the diff.

## Commands

See README § Commands. In short: `./dev` to build and serve
(http://localhost:8790/), `cargo test --workspace` for logic,
`tools/e2e.sh [--bless]` for browser goldens, `./presubmit.sh` before `main`
moves.

- Version control is **jj** (colocated git), remote `origin` =
  github.com/jdf/historyflow (private). Publish with plain jj:
  `jj bookmark set main -r @-` then `jj git push --bookmark main`.
- Never run `./deploy` without `-n` unless asked: it commits to the site repo
  (`~/projects/mrfeinberg.com`) and publishes
  https://mrfeinberg.com/historyflow/.

## The model (`crates/core`)

A data source yields `SourceVersion`s (text, author, time, comment), oldest
first. `HistoryBuilder::push` segments each text into atoms and gives every atom
a `TokenId`; an atom judged to be the same text as an earlier one reuses its
token, anything else is a new token whose origin is that version. Everything the
plot needs comes from token identity: origin gives author and age, and
`History::links` gives the passages two versions share (the ribbons).

`Config` chooses how, in three independent parts:

- `Segmenter`: `Prose` (words in sentences in paragraphs), `Sentences` (the
  original 2004 granularity), `Lines` (source code).
- `Matching`: `Historical` (WikiWho-style: whole blocks, then units, matched by
  content against the previous version and then all earlier ones, so moves and
  restored text keep their identity) or `Pairwise` (strict in-order diff against
  the previous version only, like `git blame`).
- `Measure`: token size in characters or one per atom.

Presets: `Config::prose()`, `Config::sentences_2004()`, `Config::source_lines()`
(lines + pairwise). `cargo run --release -p historyflow-core --example scale` is
a rough performance check.

`layout` turns a `History` into `Quad`s in normalised coordinates (x across
versions, y down the document, every version top-aligned): one per stretch of
same-origin text in each version's line, and one per such stretch in each link
between consecutive versions. Colour is not in the geometry: a quad carries its
author and its age, and the shader picks the colour, so changing the colouring
never rebuilds anything.

A `Bundle` is the JSON file a data source writes and the page reads: title,
suggested `Mode`, and the `SourceVersion`s with full text.

## Data sources (`crates/cli`)

Sources that cannot run in a browser are subcommands of the `historyflow`
binary, one module each, and write a bundle. `git_file` shells out to `git`
(`log --follow`, `cat-file --batch`); its tests build a real repository in a
temporary directory.

## Architecture rules

- **`crates/core`** has no browser or GL dependency. All model, diff and layout
  logic goes here so it is testable with plain `cargo test`.
- **`crates/web`** is the only crate that touches `web-sys`. Keep it thin: `App`
  holds the history and options, `renderer` draws quads as instances of a unit
  square.
- **`web/`** is deliberately vanilla JS with no bundler and no TypeScript.
  `tools/build.sh` copies it into `build/<profile>/dist` next to the
  wasm-bindgen output in `pkg/`.
- `wasm-bindgen` is pinned with `=` in `crates/web/Cargo.toml`; the CLI is
  installed to match from `Cargo.lock`. Bump the crate and the CLI follows.
- `unsafe` is forbidden workspace-wide.
- Markdown, `web/` and `tools/*.mjs` are formatted by prettier
  (`proseWrap: always`); Rust by rustfmt. Presubmit enforces both.
- Only time (x) zooms. `View` (core) is the window onto the layout's x axis,
  with the limits: the whole history at one end, three revisions at the other.
  It is a shader uniform, so zooming and panning redraw without rebuilding
  anything. y always shows the whole document.
- The crosshair's y and the text panel are linked through `passages()`: each
  passage of a version's text carries its y extent on the version's line, and
  `web/app.js` maps between that and the span's position in the scrolled text.
  Behaviour of that kind is tested in `tools/e2e-ui.mjs`, not by screenshots.
- Any rendering change needs an e2e case in `tools/e2e.sh` and a blessed golden.
  Look at the received image before blessing.

## Planned

Noted by the user for later; not started.

- **A compact representation of a history.** Bundles carry every version's full
  text, which will not scale. Design the minimal form from which the history can
  be reconstructed: deltas only. The producer could be a command-line app, a
  desktop app, or a website; its input may be very large (the whole history of a
  software repository, or of a Wikipedia article). To be designed, not yet
  specified further.
