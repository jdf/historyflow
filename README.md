# historyflow

A reimplementation of Fernanda Viégas and Martin Wattenberg's _history flow_
visualization (IBM Research, 2003) for the web: a Rust core compiled to
WebAssembly, drawing with WebGL2, inside a plain HTML and JavaScript page.

Research notes on the original are in `docs/research/`.

## Layout

| Path          | Contents                                                             |
| ------------- | -------------------------------------------------------------------- |
| `crates/core` | Platform-independent model, diff and layout. Tested natively.        |
| `crates/web`  | wasm-bindgen exports and the WebGL2 renderer.                        |
| `web/`        | The page: HTML, CSS and vanilla JavaScript. No bundler.              |
| `tools/`      | Build, dev-server and end-to-end test scripts.                       |
| `tests/`      | Approved end-to-end screenshots (`goldens/e2e`).                     |
| `build/`      | Generated: `build/<profile>/dist` is the servable site. Not tracked. |

## Requirements

- `rustup` — `rust-toolchain.toml` pins the compiler and installs the
  `wasm32-unknown-unknown` target on first use.
- Node.js and npm — for prettier, the browser test driver and `wasm-opt`.
- Python 3 — the dev server.
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
