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
#      SHOT_BACKDROP app to bring to front first, for a clean backdrop behind the tray
#                    menu (e.g. "zed"); optional, tray mode only.
set -euo pipefail

OUT="${1:-docs/screenshot.png}"
FEATURES="${2:-}"
KEYS="${3:-}"
DELAY="${SHOT_DELAY:-3}"
BACKDROP="${SHOT_BACKDROP:-}"

[ "$(uname)" = "Darwin" ] || { echo "screenshot: macOS only (needs screencapture)"; exit 1; }
mkdir -p "$(dirname "$OUT")"
BASE="${OUT%.*}"; EXT="${OUT##*.}"

# Optional clean backdrop (e.g. a blank editor) behind floating panels / the tray menu.
[ -n "$BACKDROP" ] && { osascript -e "tell application \"$BACKDROP\" to activate" 2>/dev/null || true; sleep 1; }

# Launch the app (debug build is fine; reuses the cargo cache).
run_args=(run)
[ -n "$FEATURES" ] && run_args+=(--features "$FEATURES")
cargo "${run_args[@]}" >/dev/null 2>&1 &
APP_PID=$!
disown "$APP_PID" 2>/dev/null || true   # keep the shell from printing "Terminated" on cleanup
cleanup() { pkill -P "$APP_PID" 2>/dev/null || true; kill "$APP_PID" 2>/dev/null || true; }
trap cleanup EXIT
sleep "$DELAY"

# Identify the app process by its binary name (== cargo package name). Do NOT use
# "frontmost": a `tray` build is a menu-bar accessory and never becomes frontmost.
PROC=$(cargo metadata --no-deps --format-version 1 2>/dev/null \
  | python3 -c 'import sys,json; print(json.load(sys.stdin)["packages"][0]["name"])' 2>/dev/null)
[ -n "$PROC" ] || PROC=$(osascript -e 'tell application "System Events" to get name of (first process whose frontmost is true)' 2>/dev/null)

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
  local b
  b=$(osascript -e "tell application \"System Events\" to tell process \"$PROC\" to get {position, size} of front window" 2>/dev/null | tr -d ' ')
  [ -n "$b" ] || { echo "screenshot: no front window found for process $PROC"; return 1; }
  shot "$b" "$OUT"
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
  done < <(swift "$helper" "$PROC" 2>/dev/null)
  [ "$got" = 1 ] || { echo "screenshot(overlay): no panels found for $PROC"; return 1; }
}

capture_tray() {
  # Menu-bar status item: find it, click to open its native menu, capture the corner.
  local pos sx sy rx
  pos=$(osascript -e "tell application \"System Events\" to tell process \"$PROC\" to get position of menu bar item 1 of menu bar 2" 2>/dev/null | tr -d ' ')
  [ -n "$pos" ] || { echo "screenshot(tray): no status item found for process $PROC"; return 1; }
  IFS=',' read -r sx sy <<< "$pos"
  # NB: do NOT re-activate the backdrop here — bringing another app to the front
  # right before the click suppresses the status-item menu. Rely on the backdrop
  # already being frontmost (the overlay pass, or SHOT_BACKDROP before launch).
  osascript -e "tell application \"System Events\" to tell process \"$PROC\" to click menu bar item 1 of menu bar 2" >/dev/null 2>&1 &
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
