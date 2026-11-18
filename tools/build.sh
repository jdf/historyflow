#!/usr/bin/env bash
# Builds the web app into build/<profile>/dist: the wasm crate compiled
# for wasm32-unknown-unknown, post-processed by wasm-bindgen (and wasm-opt
# for release), next to a copy of web/.
#
#   tools/build.sh [debug|release]
#
# The wasm-bindgen CLI must match the wasm-bindgen crate version exactly,
# so it is installed project-locally (build/tools) at the version
# Cargo.lock resolves, and reinstalled when that changes.
set -euo pipefail
cd "$(dirname "$0")/.."

PROFILE="${1:-debug}"
case "$PROFILE" in
  debug)   CARGO_FLAGS=() ;        BINDGEN_FLAGS=(--debug --keep-debug) ;;
  release) CARGO_FLAGS=(--release); BINDGEN_FLAGS=() ;;
  *)       echo "usage: tools/build.sh [debug|release]" >&2; exit 2 ;;
esac

cargo build -p historyflow-web --target wasm32-unknown-unknown --locked \
  ${CARGO_FLAGS[@]+"${CARGO_FLAGS[@]}"}

WANT=$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/[^0-9.]/, ""); print; exit }' Cargo.lock)
BINDGEN=build/tools/bin/wasm-bindgen
if [ "$("$BINDGEN" --version 2>/dev/null | awk '{print $2}')" != "$WANT" ]; then
  echo "== installing wasm-bindgen-cli $WANT into build/tools"
  cargo install --quiet --locked --root build/tools --version "$WANT" wasm-bindgen-cli
fi

DIST="build/$PROFILE/dist"
mkdir -p "$DIST"
"$BINDGEN" --target web --no-typescript --out-dir "$DIST/pkg" \
  ${BINDGEN_FLAGS[@]+"${BINDGEN_FLAGS[@]}"} \
  "target/wasm32-unknown-unknown/$PROFILE/historyflow_web.wasm"

if [ "$PROFILE" = release ]; then
  if [ ! -x node_modules/.bin/wasm-opt ]; then
    npm install >/dev/null
  fi
  WASM="$DIST/pkg/historyflow_web_bg.wasm"
  node_modules/.bin/wasm-opt -O3 "$WASM" -o "$WASM"
fi

# dist mirrors web/, plus the two generated entries the excludes protect.
rsync -a --delete --exclude .DS_Store --exclude pkg --exclude build-info.js \
  web/ "$DIST/"
tools/build-info.sh "$DIST/build-info.js"
