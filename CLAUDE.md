# CLAUDE.md

Guidance for Claude Code (and any agent reading this file) working in Deck.
Keep this in lockstep with `AGENTS.md` — they are the same rules. The *why*
behind every lint and gate lives in `docs/AGENTIC-ENGINEERING.md`.

## What Deck is

Deck is a native desktop-app **starter** built on GPUI + gpui-component in Rust
(macOS + Linux, rendered on Metal/wgpu). It is meant to be **forked, renamed,
and shipped** as your own app — there is no domain logic to preserve, just a
clean, working foundation.

It is a single crate (package `deck`, license `0BSD`, edition 2021). The git
GPUI stack is pinned in `Cargo.lock`; the toolchain is pinned in
`rust-toolchain.toml` to match Zed's gpui build — do not change either casually.

### `src/` layout

- `main.rs` — entry point; opens the window and wires up the app.
- `shell.rs` — the single root view; owns persisted `Settings` and routing.
- `command_palette.rs` — the ⌘K (Ctrl K) launcher, with fuzzy-match tests.
- `welcome.rs` — the Welcome page (a centered card).
- `settings.rs` — typed preferences persisted to the platform config dir.
- `settings_view.rs` — the Settings page rendered by `Shell`.
- `theme.rs` — dark/light palette with a selectable brand accent.
- `tray.rs` — optional menu-bar tray icon + dock hiding (`--features tray`).

## The loop (verify and self-correct without a CI round-trip)

- Type-check fast with `cargo check`. The full app build pulls GPUI from git, so
  the **first** build is slow; it is cached after that. Run with `just run`
  (`just run-tray` for the menu-bar variant).
- **`just ci`** runs the entire Definition of Done in one command (fmt-check +
  clippy on both feature configs + tests) — the same gate as CI, so green here
  means green there. Run it before declaring a change done.
- **`just fix`** auto-applies clippy's machine-fixable suggestions and formats.
  Loop `just fix` → `just ci` to self-correct.
- For self-correction, prefer machine-readable diagnostics:
  `cargo clippy --message-format=short` (one line each) or `--message-format=json`
  (structured spans + applicable fixes). The default human format is for people.
- Editor == CI: `.vscode/` and `.zed/` set rust-analyzer to check with **clippy**,
  so in-editor warnings match the CI lint set (rust-analyzer otherwise runs plain
  `cargo check` and would miss the `[lints.clippy]` rules).

## Definition of Done

All four must hold before you call a change done — `just ci` checks them at once.
**Paste the command output as evidence; never claim done while anything is red.**

1. `cargo fmt --all --check` is clean.
2. clippy `-D warnings` is green on **both** the default build **and**
   `--features tray` (`just check`).
3. `cargo test` is green (the `command_palette` fuzzy-match tests live here).
4. No new or changed deps in `Cargo.toml` / `Cargo.lock` unless explicitly
   approved. The git GPUI stack is bumped **only** via `just bump-gpui` —
   never hand-edit those pins.

## Code constraints

These mirror the manifest lints; internalize them before writing code:

- No `todo!()` and no `dbg!()` — both are denied.
- Never ignore a `Result` — `unused_must_use` is denied (a dropped fallible
  IO/serde result is a silent bug). Propagate with `?` or handle it.
- `unsafe_code` is denied crate-wide. A genuinely necessary `unsafe` block needs
  a reviewed `// SAFETY:` comment plus a scoped `#[allow(unsafe_code)]`.
- Deck is an app, so `.expect()` on genuinely-infallible GPUI handles is fine
  (see `main.rs` and `tray.rs`). Prefer `Result`/`?` everywhere else.

## Design work

Ground UI changes in `README.md` and `docs/LEARNINGS.md` (real screenshots and
hard-won GPUI lessons) — not remembered descriptions of how things look.
