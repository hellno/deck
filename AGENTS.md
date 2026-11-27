# AGENTS.md

Guidance for agents working on **Deck, the template**. This is the only agent guide;
`CLAUDE.md` imports it. Generated apps get their own guide from `AGENTS.md.liquid`;
for how the app itself is built and checked, read that file too.

## What Deck is

A cargo-generate template for native GPUI desktop apps (macOS + Linux) on one crates.io
dependency, `gpui-kit`. A fork generates it, renames nothing by hand, and ships.

Keep it small. Prefer deleting to adding; demos belong in a fork, not in the default app.
Comments explain a non-obvious why; docs describe what exists now. No history in either:
git has it. Commit messages: `type(scope): summary`, then why.

## This repo does not build

Files in `cargo-generate.toml`'s `include` list carry `{{ }}` Liquid tokens, so `cargo` fails
in the raw repo. Everything runs in a rendered project:

- **`just check-template`** renders a project and runs its `just ci`. That's the gate.
- **`just shot <page> <light|dark> <out.png>`** renders a project and takes a headless
  screenshot. Needs the GPU: it fails inside sandboxes such as macOS Seatbelt.
- Both render the working-tree copies of the files git tracks: unstaged edits count, but
  **new files must be staged** (`git add`) or they're missing. CI sees only what's committed.

Template rules:

- A literal that should be personalized needs its file in `include` and a token:
  `{{app_name}}` (display name), `{{project-name}}`, `{{crate_name}}`,
  `{{bundle_qualifier}}.{{bundle_org}}`. A literal in a non-included file ships verbatim.
- `*.liquid` files replace their maintainer counterparts in generated projects (README,
  AGENTS, justfile, CI, dependabot). A change to recipes, CI, or agent guidance usually
  belongs in both. Wrap Just/GitHub `{{ }}` syntax in `{% raw %}` there.
- `template-hooks/app-name.rhai` defaults the display name when none is given.
  Don't run `cargo generate --path .` from inside the repo: cargo-generate 0.23 then deletes
  the hook file from your working tree. The recipes and CI run it from outside.

## Creating a project from the template

```bash
cargo generate gh:hellno/deck --name catchy-tv -d app_name="Catchy TV"
```

`cargo generate --init` (generate into the current directory) fails in sandboxed agent
shells. Generate into a temporary directory and copy the result where it belongs.

## Upgrading GPUI

1. Bump `gpui-kit` in `Cargo.toml`. If it needs a newer Rust, bump `rust-version` there and
   `channel` in `rust-toolchain.toml` together.
2. `just update-lock -p gpui-kit`: updates `Cargo.lock` in a rendered project, runs its
   `just ci`, and copies the lock back with the root package name re-tokenized.
3. `just shot` both pages in light and dark, and launch a rendered app (`just dry-run` in it):
   some breakage only shows at runtime.
4. Check licenses: `cargo deny check licenses` runs in CI on a rendered project.

## Definition of done

1. `just check-template` is green (fmt, clippy on default and `--all-features`, tests, build).
2. UI changes: `just shot` in light and dark, and you looked at both images.
3. Docs match the code: README, `AGENTS.md.liquid`, `docs/LEARNINGS.md`.
4. No new or changed dependencies without approval.
