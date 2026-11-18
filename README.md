# historyflow

A reimplementation of Fernanda Viégas and Martin Wattenberg's _history flow_
visualization (IBM Research, 2003) for the web: a Rust core compiled to
WebAssembly, drawing with WebGL2, inside a plain HTML and JavaScript page.

Research notes on the original are in `docs/research/`.

## Viewing a file's history

The page shows a _bundle_: one JSON file holding every version of a document,
written by a data source. The first data source is one file in a git repository:

```sh
cargo run --release -p historyflow-cli -- git-file path/to/file.rs -o file.json
```

Then open the page (`./dev`) and drop `file.json` on it, or use **Open
bundle…**. The file is followed across renames, and each commit that touched it
is one version.

| Option             | Effect                                                                                |
| ------------------ | ------------------------------------------------------------------------------------- |
| `--mode lines`     | Default. Whole lines, each version diffed against the one before, like `git blame`.   |
| `--mode prose`     | Words, matched across the whole history, so moved and restored text keeps its author. |
| `--mode sentences` | Sentences, as in the original history flow.                                           |
| `--first-parent`   | Follow only the mainline; a merged branch appears as one version at its merge.        |
| `--max-versions N` | Keep only the most recent N versions.                                                 |

The mode is only the bundle's suggestion; the page's **compare** menu switches
between them. Without `--first-parent`, commits come in `git log` order, which
interleaves the two sides of a merge by date.

Click or drag on the plot to place the crosshair: the vertical line picks a
version, the horizontal line a place in it. The panel on the right shows that
version's text, coloured as in the plot and scrolled to that place, marked by an
orange line; scrolling the text moves the crosshair in turn. The left and right
arrow keys step through versions. Dots under the plot mark the other versions by
the selected version's author.

With no bundle the page shows `web/samples/demo.json`, a synthetic wiki page
written by `tools/make-sample.py`.

## Layout

| Path          | Contents                                                              |
| ------------- | --------------------------------------------------------------------- |
| `crates/core` | Platform-independent model, attribution and layout. Tested natively.  |
| `crates/cli`  | The `historyflow` command: data sources that run outside the browser. |
| `crates/web`  | wasm-bindgen exports and the WebGL2 renderer.                         |
| `web/`        | The page: HTML, CSS and vanilla JavaScript. No bundler.               |
| `tools/`      | Build, dev-server and end-to-end test scripts.                        |
| `tests/`      | Approved end-to-end screenshots (`goldens/e2e`).                      |
| `build/`      | Generated: `build/<profile>/dist` is the servable site. Not tracked.  |

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

## Deployment

`./deploy` publishes to https://mrfeinberg.com/historyflow/. It mirrors
`build/release/dist` into the `historyflow/` subtree of the site repository
(`~/projects/mrfeinberg.com`), commits that subtree there, runs the site's own
Cloudflare publish (`cloudflare/prepare.py`, then `wrangler deploy`), and pushes
the site repository. The publish is of the site's whole working tree, so any
other uncommitted edits in that repository go live with it.

`./deploy -n` runs the presubmit and lists what staging would change, without
touching the site repository.
