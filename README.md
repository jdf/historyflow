# historyflow

A reimplementation of Fernanda Viégas and Martin Wattenberg's _history flow_
visualization (IBM Research, 2003) for the web: a Rust core compiled to
WebAssembly, drawing with WebGL2, inside a plain HTML and JavaScript page.

Research notes on the original are in `docs/research/`.

## The `historyflow` command

The command carries the viewer inside it and serves it from your own machine;
nothing is fetched from anywhere else.

```sh
tools/build-cli.sh                 # builds target/release/historyflow

historyflow                        # browse the git repository you are in
historyflow path/to/repo           # or another one
historyflow src/main.rs            # a repository, opened on one file
historyflow saved.hflow            # a history file
historyflow --wikipedia https://en.wikipedia.org/wiki/Chocolate
```

Each opens your browser on the viewer. For a repository the page lists its
tracked files as a tree, folders first and closed until pressed, with a filter
that opens whatever matches. Binary files are left out, as is anything whose
name, or a folder's above it, starts with a dot; name one on the command line to
see it anyway. Choosing a file reads its history from git there and then. The
file is followed across renames, and each commit that touched it is one version.
Markdown and other prose is compared by words, and everything else by lines; the
**compare** menu changes it.

`--wikipedia` takes an article's address on any Wikipedia, or just its title on
the English one, and shows its first 500 revisions; `--revisions N` asks for
more or fewer. This is the one case where something is fetched: your browser
asks the wiki for the history, as described under
[Viewing a Wikipedia article's history](#viewing-a-wikipedia-articles-history).

`--no-open` prints the address without opening a browser, `--port` picks the
port, and `--first-parent` follows only the mainline of merges. The server
listens only on this machine, and refuses requests that were not addressed to it
by name, so that a page on another site cannot read the repository through your
browser.

## Writing a history file

The command also writes one file's history out, to keep or to pass on:

```sh
historyflow git-file path/to/file.rs -o file.hflow
```

Open it with `historyflow file.hflow`, or drop it on the page, or use **Open…**.

| Option             | Effect                                                                                |
| ------------------ | ------------------------------------------------------------------------------------- |
| `--mode lines`     | Default. Whole lines, each version diffed against the one before, like `git blame`.   |
| `--mode prose`     | Words, matched across the whole history, so moved and restored text keeps its author. |
| `--mode sentences` | Sentences, as in the original history flow.                                           |
| `--first-parent`   | Follow only the mainline; a merged branch appears as one version at its merge.        |
| `--max-versions N` | Keep only the most recent N versions.                                                 |

The mode is only the file's suggestion; the page's **compare** menu switches
between them. Without `--first-parent`, commits come in `git log` order, which
interleaves the two sides of a merge by date.

Click or drag on the plot to place the crosshair: the vertical line picks a
version, the horizontal line a place in it. The panel on the right shows that
version's text, colored as in the plot and scrolled to that place, marked by an
orange line; scrolling the text moves the crosshair in turn. The left and right
arrow keys step through versions. Dots under the plot mark the other versions by
the selected version's author.

The mouse wheel and trackpad move through time: scroll up and down (or pinch) to
zoom about the pointer, scroll sideways to pan. Zooming out stops when the whole
history fills the plot, and zooming in when three revisions do. Only time zooms;
the whole document is always in view top to bottom. While zoomed in, a scroll
thumb under the plot shows which part of the history is on screen; drag it, or
press beside it, to move.

Two menus set the vertical scale, independently of each other:

- **height** is what the plot's full height stands for: the _longest in view_
  (the default), or the _longest version_ in the whole history. With the first,
  one enormous version squeezes the rest only while it is on screen; zoom in
  beside it and the others fill the plot. The height eases to its new value as
  the view changes.
- **scale** is _linear_ or _log_. On the log scale the start of the document is
  drawn nearly in proportion and the rest progressively squeezed, so a version
  many times the usual length takes only a few times the usual height.

The **color**, **spacing**, **height** and **scale** menus are remembered in
your browser, and start as you left them. **compare** is not: each file suggests
its own.

With no file the page shows `web/samples/demo.hflow`, a synthetic wiki page
written by `tools/make-sample.py`.

## Viewing a Wikipedia article's history

Press **Wikipedia…** and give an article's title (taken to be on the English
Wikipedia) or its address on any Wikipedia, and how many revisions to fetch. Or
link straight to one: `?wiki=Chocolate&limit=500`.

The page asks the wiki itself, 50 revisions at a time starting from the
article's first, and draws the history as it arrives; **Stop** ends the fetch
early. Editors without an account are drawn in white and grays. A revision whose
text the wiki's administrators have hidden is skipped.

**Save** writes whatever is loaded as an archive (see below), so that a history
fetched once can be opened again without asking the wiki.

## History files

A history file is one of two kinds, and the page and the command read either.

An **archive** (`.hflow`) holds only what each version changed, and is what the
command and **Save** write:

- Each distinct chunk of text (a word, a run of spaces, a punctuation mark, a
  line break) is stored once, however often it occurs.
- A version is a list of instructions for making its chunks out of an earlier
  version's: copy this stretch, insert these. The earlier version is usually the
  one before; it is the one before that when that is closer (two authors
  alternating), and any earlier version that was exactly the same (a page
  restored after vandalism), which costs a few bytes.
- The whole is then deflated.

Every version's text comes back byte for byte, and nothing about authorship is
stored: the page works that out from the text, in whichever comparison is
chosen. The page keeps the loaded history in the same form, so its memory is the
size of the changes, not of the versions.

| History                                    | Versions | Text in all versions | Archive |
| ------------------------------------------ | -------- | -------------------- | ------- |
| Wikipedia's Chocolate, first 500 revisions | 500      | 7.66 MB              | 45 KB   |
| A 90 KB JavaScript file in git             | 44       | 2.09 MB              | 55 KB   |
| The demo                                   | 54       | 232 KB               | 3 KB    |

A **bundle** (`.json`) is the same history as plain JSON, every version's text
written out: easy to write from a script and to read, and as large as the texts
together. Its fields are `format` (`"historyflow-bundle/1"`), `title`, `mode`
and `versions`, each version having `id`, `author`, `anonymous`, `timestamp`
(seconds since 1970), `comment` and `text`.

```sh
historyflow convert history.json -o history.hflow   # and the other way round
```

`cargo run --release -p historyflow-core --example replay file.hflow` times
reading a file and working out its authorship in each mode.

## Layout

| Path          | Contents                                                                |
| ------------- | ----------------------------------------------------------------------- |
| `crates/core` | Platform-independent model, attribution and layout. Tested natively.    |
| `crates/cli`  | The `historyflow` command: the local server, and sources that need git. |
| `crates/web`  | wasm-bindgen exports and the WebGL2 renderer.                           |
| `web/`        | The page: HTML, CSS and vanilla JavaScript. No bundler.                 |
| `tools/`      | Build, dev-server and end-to-end test scripts.                          |
| `tests/`      | Approved end-to-end screenshots (`goldens/e2e`).                        |
| `build/`      | Generated: `build/<profile>/dist` is the servable site. Not tracked.    |

## Requirements

- `rustup` — `rust-toolchain.toml` pins the compiler and installs the
  `wasm32-unknown-unknown` target on first use.
- Node.js and npm — for prettier, the browser test driver and `wasm-opt`.
- Python 3 — the dev server.
- `git` — read by the `git-file` data source, and used by its tests.
- The site repository at `~/projects/mrfeinberg.com` and a logged-in `wrangler`
  — only for `./deploy`.

Everything else is fetched into the project on first use: the `wasm-bindgen` CLI
into `build/tools` (at the version `Cargo.lock` resolves for the crate), and a
pinned Chrome for Testing into `~/.cache/historyflow/chrome`.

## Commands

```sh
./dev                  # build debug, serve http://localhost:8790/
./dev release          # same, optimized
./dev stop             # stop the dev server

cargo test --workspace # native unit tests
tools/e2e.sh           # browser tests against the approved screenshots
tools/e2e.sh --bless   # approve the current renderings
./presubmit.sh         # formatting, clippy, tests, e2e — what CI runs

./deploy -n            # presubmit, then list what would be staged
./deploy               # presubmit, then publish to mrfeinberg.com/historyflow/
```

## Testing

Logic lives in `crates/core` and is tested with `cargo test`.

`tools/e2e.sh` loads the release build in a pinned headless Chrome that
rasterizes WebGL on the CPU (SwiftShader), fails on any console error, and
compares a screenshot of each case byte for byte with its approved image in
`tests/goldens/e2e/`. On a mismatch, compare `build/e2e-received-<name>.png`
with the golden and run `tools/e2e.sh --bless` if the change is intended. Add
cases to the `CASES` list in `tools/e2e.sh`.

It then runs `tools/e2e-ui.mjs`, which drives the page (clicks, drags,
scrolling, keys) and checks that the crosshair and the text panel stay in step.

The approved images are exact only for one Chrome version on one CPU
architecture. They are blessed on arm64 macOS; CI runs on x86-64 Linux with
`E2E_NO_GOLDEN=1`, which keeps the load and console-error checks and skips the
pixel comparison.

## Releasing the command

The command is distributed from a separate, public repository,
[jdf/historyflow-cli](https://github.com/jdf/historyflow-cli): GitHub releases
with a binary per platform. The Homebrew formula is in a third,
[jdf/homebrew-historyflow](https://github.com/jdf/homebrew-historyflow), so that
`brew install jdf/historyflow/historyflow` finds it. To release, set the version
in `Cargo.toml`, commit and push, then:

```sh
tools/release.sh 0.2.0
```

That tags this repository and starts the release workflow in the public one,
which checks this repository out with a read-only deploy key
(`SOURCE_DEPLOY_KEY` there), builds for macOS (Apple silicon and Intel) and
Linux (x86-64, static), and publishes the release. The script waits for that,
then starts the tap's own workflow, which rewrites the formula from the
release's checksums.

## Deployment

`./deploy` publishes to https://mrfeinberg.com/historyflow/. It mirrors
`build/release/dist` into the `historyflow/` subtree of the site repository
(`~/projects/mrfeinberg.com`), commits that subtree there, runs the site's own
Cloudflare publish (`cloudflare/prepare.py`, then `wrangler deploy`), and pushes
the site repository. The publish is of the site's whole working tree, so any
other uncommitted edits in that repository go live with it.

`./deploy -n` runs the presubmit and lists what staging would change, without
touching the site repository.
