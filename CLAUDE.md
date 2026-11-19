# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What this is

A reimplementation of Viégas and Wattenberg's _history flow_ visualization: Rust
→ WebAssembly → WebGL2, with a vanilla HTML/JS page. Research on the original is
in `docs/research/history-flow.md`; consult it before making design decisions
about the visual encoding or the diff.

## Commands

```sh
./dev                  # build debug, serve http://localhost:8790/
./dev release          # same, optimized
./dev stop             # stop the dev server
tools/build-cli.sh     # builds target/release/historyflow, viewer included

cargo test --workspace # native unit tests
tools/e2e.sh           # browser tests against the approved screenshots
tools/e2e.sh --bless   # approve the current renderings
./presubmit.sh         # formatting, notices, clippy, tests, e2e

./deploy -n            # presubmit, then list what would be staged
./deploy               # presubmit, then publish to mrfeinberg.com/historyflow/
```

- `README.md` is for someone using the program, and has four parts: what it is,
  how to use it, how to install it, credits. Keep everything else out of it:
  nothing about development, design decisions, or what the program does not do.
  Notes for working on the code belong in this file.
- `tools/e2e.sh` loads the release build in a pinned headless Chrome
  (SwiftShader), fails on any console error, and compares each case in its
  `CASES` list byte for byte with `tests/goldens/e2e/`; a mismatch leaves
  `build/e2e-received-<name>.png` to compare. The goldens are exact only for
  arm64 macOS; `E2E_NO_GOLDEN=1` skips the pixel comparison elsewhere. It then
  runs `tools/e2e-ui.mjs`, which drives the page and checks its behavior.
- `./deploy` mirrors `build/release/dist` into the `historyflow/` subtree of the
  site repository, commits it, runs the site's Cloudflare publish and pushes.
  The publish is of that repository's whole working tree, so check it for other
  uncommitted edits first.
- Version control is **jj** (colocated git), remote `origin` =
  github.com/jdf/historyflow (public). Publish with plain jj:
  `jj bookmark set main -r @-` then `jj git push --bookmark main`.
- **Run `./presubmit.sh` locally before a push, and do not wait for GitHub's CI
  afterwards.** The user's rule: the local presubmit is the check, and waiting
  on CI is slow and silly for a hobby project. Commit locally as work is
  finished and push when a substantial piece is complete or when asked, not
  after every small change. The browser tests hang if the laptop's lid is closed
  (headless Chrome cannot draw); that is not a failure of the code.
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

`historyflow [WHAT] [-o FILE]` has no subcommands, by the user's design: what
`WHAT` is decides what happens. An `http(s)://` address is a Wikipedia article;
otherwise it is a path: a directory is a git repository, a history file is
itself, and any other file is a file in a git repository. Without `-o` the
command serves the viewer on the loopback address and opens a browser; with it,
the history is written to a history file and nothing is opened. Do not add
subcommands or flags that restate what the argument already says.

For an article the command fetches nothing itself when serving: it tells the
page (in `api/session`) which article and how much of it. With `-o`,
`cli::wikipedia` fetches the history by running `curl` (as `git_file` runs
`git`, and to keep TLS out of the build). Its `read` takes the fetching as a
function, which is what the tests supply.

The command never writes JSON: the user will never want every version's text
written out. A bundle's JSON is read (by the command and the page) only so that
a script can make a history, as `tools/make-sample.py` does.

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
- `git_file` shells out to `git` (`log --follow`, `cat-file --batch`); tests
  build a real repository in a temporary directory (`tests/common`), and
  `tools/e2e.sh` builds one for the browser test of the command.

## Releases

`tools/release.sh <version>` tags `main`, runs `.github/workflows/release.yml`
on the tag (which builds for macOS on Apple silicon and Intel and for Linux on
x86-64, and publishes a GitHub release in this repository), then runs
`tools/formula.sh`, which writes `Formula/historyflow.rb` from the release's
checksums, and commits and pushes it. This repository is its own Homebrew tap.
Its name does not start with `homebrew-`, so the tap is added by address:
`brew tap jdf/historyflow https://github.com/jdf/historyflow`. Publishing is
outward-facing: never run it unless asked.

## The Wikipedia source

It runs in the page, since Wikipedia allows cross-origin reads. The half that
needs no network is `core::wikipedia`: `Article::parse` (title or address),
`Article::request` (the URL for the next batch) and `parse` (a response into
`SourceVersion`s and a continuation). `web/app.js` does the fetching, one
request at a time, and feeds each response to `App::add_wikipedia`, which
appends to the history; the page redraws after every batch.

Both the page and the command keep to Wikimedia's rules for unauthenticated
readers (API:Etiquette and the robot policy): one request at a time, a
`Retry-After` obeyed on a 429, and five seconds' rest after any request that
took over a second.

Tests never touch the network: `core` tests use a canned response, and
`tools/e2e-ui.mjs` replaces `fetch` for the wiki's API.

Requests name the tool and its version (`wikipedia::user_agent`), with a contact
in parentheses as Wikimedia's User-Agent policy asks. The contact is the person
running the command, not this project: `cli::wikipedia::contact` takes it from
`--contact`, `HISTORYFLOW_CONTACT` or `git config historyflow.contact`, and the
command refuses to fetch an article without one, as git refuses to commit
without a name. The command hands it to the page in `api/session`. The page on a
plain web host has no contact to give and sends the name and version alone.
Never put a default contact in the code, and never fall back to git's
`user.email`: what is sent to the wiki is the user's to choose.

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
- The About box links to `web/licenses.txt`, the license notices of every crate
  compiled into the viewer or the command. It is written by `tools/notices.py`
  and checked in; presubmit fails if it is out of date, so run the script after
  any change to the dependencies.
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
