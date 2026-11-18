#!/usr/bin/env bash
# Everything that must pass before `main` moves: formatting, lints, the
# native test suite, and the end-to-end browser tests. CI runs the same
# script.
set -euo pipefail
cd "$(dirname "$0")"

if [ ! -d node_modules/prettier ]; then
  npm install >/dev/null
fi

echo "== format (prettier: markdown, web/)"
if ! npx prettier --check "**/*.md" "web/**/*.{js,css,html}" "tools/*.mjs" >/dev/null; then
  echo "needs formatting: npx prettier --write '**/*.md' 'web/**/*.{js,css,html}' 'tools/*.mjs'" >&2
  exit 1
fi

echo "== format (rustfmt)"
cargo fmt --all --check

echo "== clippy (native + wasm)"
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy -p historyflow-web --target wasm32-unknown-unknown --locked -- -D warnings

echo "== tests"
cargo test --workspace --locked

echo "== e2e"
tools/e2e.sh

echo "== presubmit OK"
