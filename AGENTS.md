# AGENTS.md

Guidance for coding agents (Codex and others) working in Deck. Claude Code reads
the identical rules in `CLAUDE.md` — the two files are kept in lockstep. The
*why* behind every lint and gate lives in `docs/AGENTIC-ENGINEERING.md`.

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

## Performance — keep the UI thread sacred

Deck should feel instant for the same reason Linear does: **nothing on the UI hot
path waits on I/O or the network.** A native GPUI app already gets most of "how is
Linear so fast" for free — the heap is your data store, there's no bundle to split,
no reflow, no vdom diff — so these are the few rules that still need discipline.
Full contrast and rationale: `docs/LEARNINGS.md` §17.

- **Never block the render thread on I/O.** Apply changes to in-memory state and
  `cx.notify()` now; persist off the hot path — at a coarse boundary (blur/commit)
  or on `cx.background_executor()`, never on a per-keystroke `InputEvent::Change`.
  Use `Settings::save_best_effort()` for UI writes; `save()` returns the `io::Result`
  when a write is load-bearing.
- **`cx.notify()` the smallest entity that changed.** It marks the view *and its
  ancestors* dirty, so volatile state held as fields on `Shell` repaints the whole
  page. Give it its own `Entity<T>` (like `name_input` and the palette), not the root.
- **Render large lists with `uniform_list` / `list`,** never a flex column of N
  children (it rebuilds N elements + N layout nodes every frame).
- **Filter and search in memory** — the ⌘K palette already does (synchronous, no
  I/O per keystroke). Do the same for your own pickers.
- **Prefer an in-memory `Entity` over a data store** — any new store or network
  client is a new dep, which is approval-gated (Definition of Done #4).

## Design work

Ground UI changes in `README.md` and `docs/LEARNINGS.md` (real screenshots and
hard-won GPUI lessons) — not remembered descriptions of how things look.
