#!/usr/bin/env bash
# Builds the historyflow command with the viewer inside it: first the
# viewer (the page and its WebAssembly, into build/release/dist), then
# the command, which embeds that directory as it is at that moment
# (crates/cli/build.rs).
#
#   tools/build-cli.sh        # → target/release/historyflow
set -euo pipefail
cd "$(dirname "$0")/.."

tools/build.sh release
cargo build --release --locked -p historyflow-cli
echo "target/release/historyflow"
