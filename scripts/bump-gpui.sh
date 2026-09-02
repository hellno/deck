#!/usr/bin/env bash
# Refresh Deck's dependency lockfile without ever leaving a failed update behind.
# Works both in this raw cargo-generate template and in a generated fork.

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
scratch="$(mktemp -d "${TMPDIR:-/tmp}/deck-gpui.XXXXXX")"
trap 'rm -rf -- "$scratch"' EXIT

refresh_and_build() {
    cargo update

    # gpui-component occasionally has a red main commit. These opt-in pins let a
    # maintainer use the newest known-green revision without hand-editing Cargo.lock.
    if [[ -n "${GPUI_COMPONENT_REV:-}" ]]; then
        cargo update -p gpui-component --precise "$GPUI_COMPONENT_REV"
    fi
    if [[ -n "${ZED_REV:-}" ]]; then
        cargo update -p gpui --precise "$ZED_REV"
    fi

    cargo build
}

if grep -Fq 'name = "{{project-name}}"' "$repo_root/Cargo.toml"; then
    generated_name="deck-lock-refresh"
    cargo generate \
        --path "$repo_root" \
        --name "$generated_name" \
        --silent \
        --vcs none \
        --destination "$scratch"

    (
        cd "$scratch/$generated_name"
        refresh_and_build
    )

    # cargo-generate replaces the root package name in Cargo.lock. Put the Liquid
    # token back before atomically installing the verified lockfile in the template.
    awk -v generated_name="$generated_name" '
        !replaced && $0 == "name = \"" generated_name "\"" {
            print "name = \"{{project-name}}\""
            replaced = 1
            next
        }
        { print }
        END { if (!replaced) exit 1 }
    ' "$scratch/$generated_name/Cargo.lock" > "$scratch/Cargo.lock.normalized"
    mv "$scratch/Cargo.lock.normalized" "$repo_root/Cargo.lock"
else
    cp "$repo_root/Cargo.lock" "$scratch/Cargo.lock.backup"

    # Run with errexit in a subshell while retaining control here so a failed
    # dependency build can restore the caller's exact pre-existing lockfile.
    set +e
    (
        set -e
        cd "$repo_root"
        refresh_and_build
    )
    status=$?
    set -e

    if ((status != 0)); then
        cp "$scratch/Cargo.lock.backup" "$repo_root/Cargo.lock"
        echo "Dependency update failed; restored the previous Cargo.lock." >&2
        exit "$status"
    fi
fi

echo "→ Dependencies refreshed and the default build is green. Run just ci, then smoke-test the app."
