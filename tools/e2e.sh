#!/usr/bin/env bash
# End-to-end tests: loads the real release build in headless Chrome on
# SwiftShader-backed ANGLE (Chrome's WebGL stack, minus the GPU), fails on
# any console error, and byte-compares screenshots against the approved
# goldens in tests/goldens/e2e/.
#
#   tools/e2e.sh          # verify
#   tools/e2e.sh --bless  # approve the current renderings
#
# Chrome is pinned (tools/chrome-version.txt) so goldens never drift with
# browser updates; bump the pin and re-bless deliberately. Goldens are
# bit-exact per architecture: they are blessed on arm64 macOS, so CI
# (x86-64 Linux) runs with E2E_NO_GOLDEN=1, which keeps the load and
# console-error checks and skips the pixel comparison.
set -euo pipefail
cd "$(dirname "$0")/.."

PORT=8791
GOLDEN_DIR=tests/goldens/e2e

# name|query — add a case per view/mode as the visualization grows.
CASES=(
  "demo-author-version|?no-ui"
  "demo-age-date|?no-ui&coloring=age&spacing=date"
  "demo-sentences|?no-ui&mode=sentences"
  "demo-ui|?e2e"
  "demo-ui-cursor|?e2e&at=30,0.35"
  "demo-zoomed|?no-ui&at=24,0.3&view=0.35,0.2"
  "demo-fit-view|?no-ui&at=8,0.1&view=0,0.25&height=view"
  "demo-log|?no-ui&at=30,0.35&scale=log"
)

# The viewer, and the command with the viewer inside it.
tools/build-cli.sh >/dev/null
if [ ! -d node_modules/puppeteer-core ]; then
  npm install >/dev/null
fi
CHROME="${CHROME:-$(tools/get-chrome.sh)}"

# The harness owns this port; a leftover listener could serve a stale
# dist. Kill only the LISTENER, never clients connected to the port.
lsof -ti :"$PORT" -sTCP:LISTEN 2>/dev/null | xargs kill 2>/dev/null || true

python3 tools/serve.py "$PWD/build/release/dist" "$PORT" &
SERVER_PID=$!
trap 'kill $SERVER_PID 2>/dev/null' EXIT
for _ in $(seq 1 50); do
  curl -sf -o /dev/null "http://localhost:$PORT/" && break
  sleep 0.1
done

FAILED=0
VERIFIED=0
while IFS= read -r line; do
  NAME="${line#shot: }"
  if [ "$NAME" = "$line" ]; then
    echo "$line" >&2  # driver errors
    continue
  fi
  VERIFIED=$((VERIFIED + 1))
  GOLDEN="$GOLDEN_DIR/$NAME.png"
  RECEIVED="build/e2e-received-$NAME.png"

  if [ "${1:-}" = "--bless" ]; then
    mkdir -p "$GOLDEN_DIR"
    cp "$RECEIVED" "$GOLDEN"
    echo "blessed: $GOLDEN ($(wc -c < "$GOLDEN" | tr -d ' ') bytes)"
  elif [ -n "${E2E_NO_GOLDEN:-}" ]; then
    echo "e2e loaded OK (golden skipped): $NAME"
  elif [ ! -f "$GOLDEN" ]; then
    echo "no golden at $GOLDEN — run with --bless after verifying $RECEIVED" >&2
    FAILED=1
  elif cmp -s "$GOLDEN" "$RECEIVED"; then
    echo "e2e golden OK: $NAME"
  else
    echo "e2e golden MISMATCH: compare $RECEIVED against $GOLDEN" >&2
    FAILED=1
  fi
done < <(node tools/e2e.mjs "$CHROME" "http://localhost:$PORT/" build \
  "${CASES[@]}")

# A process substitution's exit status is invisible to set -e; a failed
# or dead driver shows up as a short shot count.
if [ "$VERIFIED" -ne "${#CASES[@]}" ]; then
  echo "e2e driver produced $VERIFIED of ${#CASES[@]} shots" >&2
  FAILED=1
fi
# Behaviour the screenshots cannot show: the crosshair, the text panel
# and the keyboard staying in step.
# …and the historyflow command serving a repository: a small one made
# here, the same every time.
REPO=build/e2e-repo
COMMAND_PORT=8792
rm -rf "$REPO" && mkdir -p "$REPO/src"
commit() {  # author, seconds since 1970, message
  git -C "$REPO" add -A
  GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_SYSTEM=/dev/null \
    GIT_AUTHOR_NAME="$1" GIT_AUTHOR_EMAIL=author@example.com GIT_AUTHOR_DATE="@$2 +0000" \
    GIT_COMMITTER_NAME="$1" GIT_COMMITTER_EMAIL=author@example.com GIT_COMMITTER_DATE="@$2 +0000" \
    git -C "$REPO" commit -q -m "$3"
}
git -C "$REPO" init -q -b main
printf 'fn main() {\n    one();\n}\n' > "$REPO/src/main.rs"
printf '# Notes\n\nSome words here.\n' > "$REPO/notes.md"
commit ada 1000000000 "Start"
printf 'fn main() {\n    one();\n    two();\n}\n' > "$REPO/src/main.rs"
commit brooke 1000086400 "Add two"
printf 'fn main() {\n    two();\n}\n\nfn two() {}\n' > "$REPO/src/main.rs"
commit ada 1000172800 "Drop one, define two"
lsof -ti :"$COMMAND_PORT" -sTCP:LISTEN 2>/dev/null | xargs kill 2>/dev/null || true
target/release/historyflow "$REPO" --no-open --port "$COMMAND_PORT" 2>/dev/null &
COMMAND_PID=$!
trap 'kill $SERVER_PID $COMMAND_PID 2>/dev/null' EXIT
for _ in $(seq 1 50); do
  curl -sf -o /dev/null "http://127.0.0.1:$COMMAND_PORT/" && break
  sleep 0.1
done

if node tools/e2e-ui.mjs "$CHROME" "http://localhost:$PORT/" "http://127.0.0.1:$COMMAND_PORT/"; then
  echo "e2e ui OK"
else
  FAILED=1
fi
exit $FAILED
