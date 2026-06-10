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

# Regenerate assets/icon.png + assets/icon.icns from assets/icon.svg.
# Needs cairosvg (pip install cairosvg); falls back to qlmanage if missing.
# Uses only macOS built-ins (sips, iconutil) for the .icns step.
icon:
    #!/usr/bin/env bash
    set -euo pipefail
    cd assets
    if command -v cairosvg >/dev/null; then
        cairosvg icon.svg -o icon.png -W 1024 -H 1024
    else
        qlmanage -t -s 1024 -o . icon.svg >/dev/null && mv icon.svg.png icon.png
    fi
    rm -rf icon.iconset && mkdir icon.iconset
    for sz in 16 32 64 128 256 512; do
        sips -z $sz $sz       icon.png --out icon.iconset/icon_${sz}x${sz}.png   >/dev/null
        sips -z $((sz*2)) $((sz*2)) icon.png --out icon.iconset/icon_${sz}x${sz}@2x.png >/dev/null
    done
    sips -z 1024 1024 icon.png --out icon.iconset/icon_512x512@2x.png >/dev/null
    iconutil -c icns icon.iconset -o icon.icns
    rm -rf icon.iconset
    echo "→ assets/icon.png + assets/icon.icns regenerated"
