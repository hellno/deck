# Agentic engineering: lints, gates and loops for building with coding agents

The *why* behind this project's lint and CI setup. The *what to run* is in `AGENTS.md`.

A coding agent is fast, has no memory of this repo's intent, and will report "done" while
something is red. The cheapest fix is to let the compiler and CI say no:

1. **The manifest is the source of truth.** Lints live in `Cargo.toml`, so rust-analyzer and
   `cargo check` show the same policy CI enforces while the code is being written.
2. **One gate, run everywhere.** `just ci` is the Definition of Done locally, and CI runs
   exactly `just ci` (plus cargo-deny), so the two can't drift.
3. **Compile time over runtime.** Anything that can fail at launch gets a test (shortcut
   parsing) or a build step (`cargo build` in `just ci`, so a stale binary isn't what gets run).

## Lints (`[lints]` in `Cargo.toml`)

| Lint | Level | Why |
|---|---|---|
| `clippy::all` | warn, priority -1 | warn locally, `-D warnings` in CI: a toolchain bump that adds lints can't break `cargo build` |
| `clippy::todo`, `clippy::dbg_macro` | deny | a stray `todo!()` panics in production; `dbg!` is debugging noise |
| `unused_must_use` | deny | a dropped `Result` from IO/serde is a silent bug |
| `unsafe_code` | deny | not `forbid`: deny keeps a reviewed escape hatch (`// SAFETY:` + scoped `#[allow]`) |

No `clippy.toml` or `rustfmt.toml`: the defaults are what this project uses, and clippy reads the
MSRV from `rust-version`. Add them when you need a setting (e.g. `disallowed-methods` with a
reason, the highest-signal clippy tool for "never call X here").

## Gates

- **`just ci`**: `cargo fmt --check`, clippy `-D warnings` on default and `--all-features`,
  `cargo test --all-features`, and `cargo build` of both. All with `--locked`, so CI never
  silently re-resolves.
- **cargo-deny** in CI, with `--all-features` so the tray's dependencies count too:
  vulnerability advisories (anywhere in the tree), a permissive license allow list, crates.io
  as the only source, no wildcard versions.
- **Feature sets:** there's one feature (`tray`), so default + `--all-features` covers both.
  Add a leg only if you add a feature whose combination matters.

## The local loop

- `cargo check` for speed, `just fix` for clippy's machine-fixable suggestions, `just ci` to finish.
- `cargo clippy --message-format=short` gives one-line diagnostics that are easy to act on.
- `.vscode/` and `.zed/` set rust-analyzer to check with clippy, so the editor matches CI.
- UI work: `just shot` renders the real views headlessly; look at the PNG instead of guessing.
  `just dry-run` for the live app. Never drive the app with global keystrokes (see
  `docs/LEARNINGS.md` §14).

## One agent guide

`AGENTS.md` is the guide; `CLAUDE.md` is a one-line `@AGENTS.md` import. Two copies of the
same rules drift, and keeping them in lockstep costs an edit in every commit that touches them.

## Deliberately not done

| Tempting | Why not |
|---|---|
| `#![forbid(unsafe_code)]` | no escape hatch if a platform call ever needs `unsafe` |
| `clippy::pedantic` / `nursery` / `restriction` wholesale | a one-off run on this app gave ~40 suggestions, all style (`must_use`, `Self`, `mul_add`, doc sections), no bugs; nursery changes between toolchains; restriction contradicts itself. Cherry-pick. |
| `[profile.dev.package."*"] debug = false` | shrinks a clean dev `target/` by ~30% (2.6 → 1.8 GB), same build time, but you can't step into GPUI in a debugger. Add it if disk matters more. |
| `multiple-versions = "deny"` | the cross-platform GPUI graph has many target-specific duplicates; advisories still gate every version |
| `panic = "abort"` | loses unwinding backtraces for little gain in a desktop app |
| Alternate linkers (mold/lld) | per-machine setup for every fork and runner; Apple's default linker is fine |
| `unwrap_used` / `expect_used` denies | `.expect()` on an infallible GPUI call is correct; adopt them in a GPUI-free core crate if you build something security-sensitive |
