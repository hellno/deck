#!/usr/bin/env bash
# Screenshot the app this repo builds. macOS only (built-in `screencapture` + `osascript`).
#
# One-time setup (the OS guards screen capture + UI scripting behind permissions):
#   System Settings -> Privacy & Security -> Screen Recording -> enable your terminal
#   System Settings -> Privacy & Security -> Accessibility     -> enable your terminal
# A sandboxed/headless agent session usually lacks both and can't self-grant them.
#
# Usage:  scripts/screenshot.sh [OUT] [FEATURES] [KEYS]
#   OUT       output image path           (default: docs/screenshot.png)
#   FEATURES  cargo features to build/run  (e.g. overlay, tray; default: none)
#   KEYS      shortcut to drive the app first, window mode only
#             (e.g. cmd+k for the palette, cmd+, for settings)
#
# What gets captured is decided by FEATURES (each handled independently, so
# `tray,overlay` captures both):
#   (none)           the front app window, after optional KEYS            -> OUT
#   contains overlay the floating rail + pill panels, alpha (transparent)  -> OUT -rail/-pill
#   contains tray    the menu-bar status item with its menu open           -> OUT
#
# Overlay capture is by window id with the alpha channel (no background), so it can't
# leak whatever is behind the panels and needs no clean backdrop. The tray menu is
# opaque, but its region capture may include whatever is behind it — pass SHOT_BACKDROP
# (a blank editor) or crop the result.
#
# Env: SHOT_DELAY    seconds to wait for first paint (default 3)
#      SHOT_TIMEOUT  maximum seconds to wait for the built app to launch (default 120)
#      SHOT_BACKDROP app to bring to front first, for a clean backdrop behind the tray
#                    menu (e.g. "zed"); optional, tray mode only.
set -euo pipefail

OUT="${1:-docs/screenshot.png}"
FEATURES="${2:-}"
KEYS="${3:-}"
DELAY="${SHOT_DELAY:-3}"
TIMEOUT="${SHOT_TIMEOUT:-120}"
BACKDROP="${SHOT_BACKDROP:-}"

[ "$(uname)" = "Darwin" ] || { echo "screenshot: macOS only (needs screencapture)"; exit 1; }
mkdir -p "$(dirname "$OUT")"
BASE="${OUT%.*}"; EXT="${OUT##*.}"

# Optional clean backdrop (e.g. a blank editor) behind floating panels / the tray menu.
[ -n "$BACKDROP" ] && { osascript -e "tell application \"$BACKDROP\" to activate" 2>/dev/null || true; sleep 1; }

# Identify the app process by its binary target name. Do NOT use
# "frontmost": a `tray` build is a menu-bar accessory and never becomes frontmost.
PROC=$(cargo metadata --no-deps --format-version 1 2>/dev/null \
  | python3 -c 'import sys,json; p=json.load(sys.stdin)["packages"][0]; print(next(t["name"] for t in p["targets"] if "bin" in t["kind"]))' 2>/dev/null)
TARGET_DIR=$(cargo metadata --no-deps --format-version 1 2>/dev/null \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["target_directory"])' 2>/dev/null)

# Build first, then launch the exact binary ourselves. `cargo run` may exec the
# binary or spawn it depending on Cargo/platform details, which makes PID-based
# window capture race-prone.
build_args=(build)
[ -n "$FEATURES" ] && build_args+=(--features "$FEATURES")
cargo "${build_args[@]}"
RUN_LOG=$(mktemp "${TMPDIR:-/tmp}/deck-screenshot-run.XXXXXX")
"$TARGET_DIR/debug/$PROC" >"$RUN_LOG" 2>&1 &
APP_PID=$!
BINARY_PID=$APP_PID
disown "$APP_PID" 2>/dev/null || true   # keep the shell from printing "Terminated" on cleanup
cleanup() {
  pkill -P "$APP_PID" 2>/dev/null || true
  kill "$APP_PID" 2>/dev/null || true
  rm -f -- "$RUN_LOG"
}
trap cleanup EXIT
ready=0
for ((attempt = 0; attempt < TIMEOUT * 4; attempt++)); do
  if osascript \
      -e "tell application \"System Events\" to exists (first process whose unix id is $BINARY_PID)" \
      2>/dev/null | grep -q true; then
    ready=1
    break
  fi
  if ! kill -0 "$APP_PID" 2>/dev/null; then
    echo "screenshot: $PROC exited before its windows became available" >&2
    cat "$RUN_LOG" >&2
    exit 1
  fi
  sleep 0.25
done
[ "$ready" = 1 ] || {
  echo "screenshot: timed out after ${TIMEOUT}s waiting for process $PROC" >&2
  cat "$RUN_LOG" >&2
  exit 1
}
sleep "$DELAY"

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
shot() { screencapture -x -o -R"$1" "$2" && echo "-> $2"; }

capture_window() {
  if [ -n "$KEYS" ]; then  # optionally drive to a view (cmd+k, cmd+,, ...)
    local key="${KEYS##*+}" mods="" parts m
    IFS='+' read -ra parts <<< "$KEYS"
    for m in "${parts[@]:0:${#parts[@]}-1}"; do
      case "$m" in
        cmd|command) mods+="command down, " ;;
        shift) mods+="shift down, " ;;
        ctrl|control) mods+="control down, " ;;
        opt|option|alt) mods+="option down, " ;;
      esac
    done
    if [ -n "$mods" ]; then
      osascript -e "tell application \"System Events\" to keystroke \"$key\" using {${mods%, }}"
    else
      osascript -e "tell application \"System Events\" to keystroke \"$key\""
    fi
    sleep 0.7
  fi
  local helper="$SCRIPT_DIR/winid.swift" id
  { command -v swift >/dev/null && [ -f "$helper" ]; } || {
    echo "screenshot: needs swift + scripts/winid.swift"; return 1;
  }
  # Window-id capture is exact on Retina/scaled displays; region capture mixes
  # AppleScript points with screencapture pixels and can include the desktop.
  id=$(swift "$helper" "$BINARY_PID" 2>/dev/null \
    | awk '$2 * $3 > area { area = $2 * $3; id = $1 } END { print id }')
  [ -n "$id" ] || { echo "screenshot: no front window found for process $PROC"; return 1; }
  screencapture -x -o -l"$id" "$OUT"
  echo "-> $OUT"
}

capture_overlay() {
  # Capture each floating panel by its window id WITH its alpha channel
  # (`screencapture -l`), so the shot has a transparent background and can't leak
  # whatever is behind it — no clean backdrop needed. Needs `swift` + winid.swift.
  # rail = portrait (h>w), pill = landscape (w>h); the big main window is skipped.
  local helper="$SCRIPT_DIR/winid.swift" got=0 id w h tag f
  { command -v swift >/dev/null && [ -f "$helper" ]; } || { echo "screenshot(overlay): needs swift + scripts/winid.swift"; return 1; }
  while read -r id w h; do
    { [ "$w" -lt 500 ] && [ "$h" -lt 500 ]; } || continue   # skip the big main window
    tag=$([ "$h" -gt "$w" ] && echo rail || echo pill)
    f="${BASE}-${tag}.${EXT}"
    screencapture -x -o -l"$id" "$f" || continue
    # Trim the transparent margins down to the panel (no-op if Pillow isn't installed).
    python3 - "$f" <<'PY' 2>/dev/null || true
import sys
from PIL import Image
im = Image.open(sys.argv[1]).convert("RGBA"); b = im.getbbox()
if b: im.crop(b).save(sys.argv[1])
PY
    echo "-> $f"; got=1
  done < <(swift "$helper" "$BINARY_PID" 2>/dev/null)
  [ "$got" = 1 ] || { echo "screenshot(overlay): no panels found for $PROC"; return 1; }
}

capture_tray() {
  # Menu-bar status item: find it, click to open its native menu, capture the corner.
  local pos sx rx
  pos=$(osascript -e "tell application \"System Events\" to tell (first process whose unix id is $BINARY_PID) to get position of menu bar item 1 of menu bar 2" 2>/dev/null | tr -d ' ')
  [ -n "$pos" ] || { echo "screenshot(tray): no status item found for process $PROC"; return 1; }
  IFS=',' read -r sx _ <<< "$pos"
  # NB: do NOT re-activate the backdrop here — bringing another app to the front
  # right before the click suppresses the status-item menu. Rely on the backdrop
  # already being frontmost (the overlay pass, or SHOT_BACKDROP before launch).
  osascript -e "tell application \"System Events\" to tell (first process whose unix id is $BINARY_PID) to click menu bar item 1 of menu bar 2" >/dev/null 2>&1 &
  local click=$!; sleep 1.2
  rx=$(( sx - 110 < 0 ? 0 : sx - 110 ))
  shot "${rx},0,300,170" "$OUT"
  osascript -e 'tell application "System Events" to key code 53' 2>/dev/null || true   # Esc closes the menu
  kill "$click" 2>/dev/null || true
}

did=0
case "$FEATURES" in *overlay*) capture_overlay && did=1 ;; esac
case "$FEATURES" in *tray*)    capture_tray    && did=1 ;; esac
[ "$did" = 0 ] && capture_window
