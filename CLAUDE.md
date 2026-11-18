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
- **Do not push after every small change.** Every push to `main` runs CI, and
  the user has asked that it not run for each small change. Commit locally as
  work is finished, check it with `./presubmit.sh`, and push when a substantial
  piece of work is complete or when asked. After a push, wait for CI's result
  before calling it good.
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

`layout` turns a `History` into `Quad`s in normalized coordinates (x across
versions, y down the document, every version top-aligned): one per stretch of
same-origin text in each version's line, and one per such stretch in each link
between consecutive versions. Color is not in the geometry: a quad carries its
author and its age, and the shader picks the color, so changing the coloring
never rebuilds anything.

An `Archive` is how a history is held and stored: the `SourceVersion`s, with
title and suggested `Mode`, kept as deltas. Each distinct chunk of text is
stored once, and each version is copy/insert instructions against an earlier one
(the previous, the one before it, or an identical earlier version). It is
lossless and knows nothing of authorship: it is read back into text, which a
`HistoryBuilder` then attributes. `to_bytes`/`from_bytes` are the `.hflow` file
(the same instructions, deflated); `from_bytes` trusts nothing in the file.

A `Bundle` is the same history as plain JSON with every version's text written
out. Sources produce one; `Archive::read` accepts either kind of file.

Why storage and attribution are separate: stored token identities would tie
files to one comparison mode and to the attribution algorithm as it stood when
they were written. Re-attributing is cheap (40 ms natively for 500 revisions of
a Wikipedia article); `--example replay` measures it.

## The command (`crates/cli`)

`historyflow [PATH]` serves the viewer on the loopback address and opens a
browser; `PATH` is a git repository, a file in one, or a history file.

- The viewer is inside the binary: `build.rs` embeds `build/release/dist` as it
  is when the command is compiled (an empty list if it has not been built, in
  which case the command says so when asked to serve). Always build with
  `tools/build-cli.sh`, which builds the viewer first.
- `serve::reply` is the whole server as a function from a request to a reply,
  which is what the tests call; `serve::serve` wraps it in `tiny_http`. It
  refuses any request whose `Host` is not this machine and port, so that a page
  on another site cannot read the repository through the browser.
- It serves `index.html` with a `historyflow-server` meta tag added. The page
  asks `api/session` only when it finds that tag; on a plain host it never asks,
  so nothing 404s. `api/history?path=` reads one file's history from git
  (`git_file`) and returns it as an archive.
- `git-file` and `convert` are the subcommands that write files. `git_file`
  shells out to `git` (`log --follow`, `cat-file --batch`); tests build a real
  repository in a temporary directory (`tests/common`), and `tools/e2e.sh`
  builds one for the browser test of the command.

## Releases

`tools/release.sh <version>` tags this repository and dispatches `release.yml`
in the public `jdf/historyflow-cli` (cloned at `~/projects/historyflow-cli`),
which builds from here with a read-only deploy key and publishes the release and
`Formula/historyflow.rb` there. Publishing is outward-facing: never run it
unless asked.

## The Wikipedia source

It runs in the page, since Wikipedia allows cross-origin reads. The half that
needs no network is `core::wikipedia`: `Article::parse` (title or address),
`Article::request` (the URL for the next batch) and `parse` (a response into
`SourceVersion`s and a continuation). `web/app.js` does the fetching, one
request at a time, and feeds each response to `App::add_wikipedia`, which
appends to the history; the page redraws after every batch.

Tests never touch the network: `core` tests use a canned response, and
`tools/e2e-ui.mjs` replaces `fetch` for the wiki's API. Requests carry an
`Api-User-Agent` header naming only the tool and its version.

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
- American spelling everywhere: code, comments, docs, UI text and commit
  messages (color, gray, license, behavior, normalize).
- Markdown, `web/` and `tools/*.mjs` are formatted by prettier
  (`proseWrap: always`); Rust by rustfmt. Presubmit enforces both.
- Only time (x) zooms. `View` (core) is the window onto the layout's x axis,
  with the limits: the whole history at one end, three revisions at the other.
  It is a shader uniform, so zooming and panning redraw without rebuilding
  anything. y always shows the whole document.
- The vertical scale is `YScale` (core): `top`, the layout y at the plot's
  bottom (1, or the longest version in view), and `log`, a steepness (0 for
  linear). It too is a shader uniform; the shader's `curve` must stay the same
  function as `YScale::down`, which the overlay and picking use.
- The scale's `top` eases toward its target (`YScale::eased_toward`):
  `App::draw` takes the time and returns whether it is still moving, and the
  page keeps drawing until it is not. Picking and the overlay use the scale as
  drawn, mid-ease included. A newly loaded history, a link with `view=`, and a
  reader who prefers reduced motion all start at rest (`App::settle`).
- The crosshair's y and the text panel are linked through `passages()`: each
  passage of a version's text carries its y extent on the version's line, and
  `web/app.js` maps between that and the span's position in the scrolled text.
  Behavior of that kind is tested in `tools/e2e-ui.mjs`, not by screenshots.
- Any rendering change needs an e2e case in `tools/e2e.sh` and a blessed golden.
  Look at the received image before blessing.
