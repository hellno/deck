# Deck template maintainer recipes. This raw repo carries {{ }} tokens and does not build;
# each recipe renders a throwaway project first. Generated apps get justfile.liquid instead.
# Needs: cargo-generate (`cargo install cargo-generate`) and just.

default:
    @just --list

# Render the template (working-tree copies of git-tracked files) and run the app's `just ci`.
check-template:
    #!/usr/bin/env bash
    set -euo pipefail
    dir="$(mktemp -d "${TMPDIR:-/tmp}/deck-check.XXXXXX")"
    trap 'rm -rf -- "$dir"' EXIT
    just --justfile "{{justfile()}}" _render "$dir" app
    cd "$dir/app"
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-{{justfile_directory()}}/target/template-check}"
    just ci

# Headless screenshot of the rendered app, e.g. `just shot settings dark docs/screenshot-settings.png`.
shot page appearance out:
    #!/usr/bin/env bash
    set -euo pipefail
    out={{quote(out)}}
    mkdir -p "$(dirname "$out")"
    out="$(cd "$(dirname "$out")" && pwd)/$(basename "$out")"
    dir="$(mktemp -d "${TMPDIR:-/tmp}/deck-shot.XXXXXX")"
    trap 'rm -rf -- "$dir"' EXIT
    just --justfile "{{justfile()}}" _render "$dir" deck
    cd "$dir/deck"
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-{{justfile_directory()}}/target/template-check}"
    cargo run --quiet --example shot -- {{quote(page)}} {{quote(appearance)}} "$out"

# `cargo update <args>` (e.g. `-p gpui-kit`) in a rendered project, gate with `just ci`, copy the lock back.
update-lock *args:
    #!/usr/bin/env bash
    set -euo pipefail
    dir="$(mktemp -d "${TMPDIR:-/tmp}/deck-lock.XXXXXX")"
    trap 'rm -rf -- "$dir"' EXIT
    just --justfile "{{justfile()}}" _render "$dir" app
    cd "$dir/app"
    export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-{{justfile_directory()}}/target/template-check}"
    cargo update {{args}}
    just ci
    sed 's/^name = "app"$/name = "{{{{project-name}}"/' Cargo.lock > "{{justfile_directory()}}/Cargo.lock"
    echo "Cargo.lock updated"

# Regenerate assets/icon.png + assets/icon.icns from assets/icon-source.png (Pillow + iconutil).
icon:
    python3 scripts/make-app-icon.py

# Render into <dest>/<name>. Copies the working-tree bytes of files git tracks (so unstaged
# edits count; new files must be staged; no target/ or editor state), and
# runs cargo-generate from outside the template: run from inside, cargo-generate 0.23
# deletes the template's own hook file.
_render dest name:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p "{{dest}}/template"
    cd "{{justfile_directory()}}"
    git ls-files | while IFS= read -r f; do [ -e "$f" ] && printf '%s\0' "$f"; done \
        | tar --null -T - -cf - | tar -xf - -C "{{dest}}/template"
    cd "{{dest}}"
    cargo generate --path "{{dest}}/template" --name "{{name}}" --silent --vcs none --destination "{{dest}}"
