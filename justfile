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

# Run with the floating overlay surface (transparent always-on-top window).
run-overlay:
    cargo run --features overlay

# Format the code.
fmt:
    cargo fmt

# Lint both feature configurations (clippy, warnings = errors). For the FULL gate use `just ci`.
check:
    cargo clippy --locked --all-targets -- -D warnings
    cargo clippy --locked --all-targets --features tray -- -D warnings
    cargo clippy --locked --all-targets --features overlay -- -D warnings
    cargo clippy --locked --all-targets --features tray,overlay -- -D warnings

# The full CI gate, locally — the whole Definition of Done in one command. Run this before you
# call a change done; it mirrors .github/workflows/ci.yml so green here == green in CI.
ci:
    cargo fmt --all --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo clippy --locked --all-targets --features tray -- -D warnings
    cargo clippy --locked --all-targets --features overlay -- -D warnings
    cargo clippy --locked --all-targets --features tray,overlay -- -D warnings
    cargo test --locked
    cargo test --locked --features overlay

# Auto-fix everything fixable: clippy's machine-applicable suggestions + formatting.
# Re-run `just ci` afterwards to confirm green. (--allow-dirty so it works mid-edit.)
fix:
    cargo clippy --fix --allow-dirty --allow-staged --all-targets
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --features tray
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --features overlay
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --features tray,overlay
    cargo fmt

# Bump the git GPUI stack to the latest upstream commits, then rebuild.
# Reproducibility lives in Cargo.lock — commit it (and rust-toolchain.toml if you
# bumped it) after this succeeds. If the build fails on an unstable-feature error,
# match rust-toolchain.toml to Zed's: https://github.com/zed-industries/zed/blob/main/rust-toolchain.toml
# Full procedure + the crates.io fallback channel: docs/UPGRADING.md
bump-gpui:
    cargo update -p gpui -p gpui_platform -p gpui-component -p gpui-component-assets
    cargo build
    @echo "→ Bumped. Run the app to smoke-test, then commit Cargo.lock (+ rust-toolchain.toml if changed)."

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
