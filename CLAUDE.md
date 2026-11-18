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

- Version control is **jj** (colocated git). Commit finished work; the user
  pushes and deploys themselves.
- Never run `./deploy` without `-n` unless asked: it publishes.

## Architecture rules

- **`crates/core`** has no browser or GL dependency. All model, diff and layout
  logic goes here so it is testable with plain `cargo test`.
- **`crates/web`** is the only crate that touches `web-sys`. Keep it thin.
- **`web/`** is deliberately vanilla JS with no bundler and no TypeScript.
  `tools/build.sh` copies it into `build/<profile>/dist` next to the
  wasm-bindgen output in `pkg/`.
- `wasm-bindgen` is pinned with `=` in `crates/web/Cargo.toml`; the CLI is
  installed to match from `Cargo.lock`. Bump the crate and the CLI follows.
- `unsafe` is forbidden workspace-wide.
- Markdown, `web/` and `tools/*.mjs` are formatted by prettier
  (`proseWrap: always`); Rust by rustfmt. Presubmit enforces both.
- Any rendering change needs an e2e case in `tools/e2e.sh` and a blessed golden.
  Look at the received image before blessing.
