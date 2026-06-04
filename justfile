# Deck — task runner. Install `just`: brew install just
# (Everything here is plain cargo + macOS built-ins; you can run the commands by hand too.)

# List available recipes.
default:
    @just --list

# Run the app (debug). This is the one you'll use 99% of the time.
run:
    cargo run

# Run optimized.
run-release:
    cargo run --release

# Run as a menu-bar / tray app (no dock icon).
run-tray:
    cargo run --features tray

# Format + lint (both feature configurations).
fmt:
    cargo fmt
check:
    cargo clippy --all-targets -- -D warnings
    cargo clippy --all-targets --features tray -- -D warnings

# Build a distributable Deck.app (needs: cargo install cargo-bundle).
# Output: target/release/bundle/osx/Deck.app
bundle:
    cargo bundle --release
    @echo "→ target/release/bundle/osx/Deck.app"

# Open the bundled app.
open: bundle
    open "target/release/bundle/osx/Deck.app"

# Regenerate assets/icon.png + assets/icon.icns from assets/icon-source.png.
# Drop your own 1024x1024 image at assets/icon-source.png first (any square art —
# a render, photo, or logo; an .svg source is rasterized automatically).
# Bakes in the macOS squircle tile + padding + shadow (macOS does NOT auto-mask).
# Needs Pillow (pip install pillow); the .icns step uses macOS iconutil.
icon:
    python3 scripts/make-app-icon.py

# Same, plus a freedesktop hicolor tree + .desktop entry under dist/linux/.
icon-linux:
    python3 scripts/make-app-icon.py --linux --web
