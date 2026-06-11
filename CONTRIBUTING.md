# Contributing to Deck

Deck is a small, opinionated **starter** (`0BSD`) — fork it, rename it, ship it.
There's no domain logic to preserve, so the bar for upstream changes is "does it
make the starter a better foundation for everyone who forks it?"

You don't need to read this to *use* Deck — fork and go. This is for changes you
want to land back **upstream**.

## What's in scope

Good upstream contributions:

- **Linux fixes & polish** — the author daily-drives macOS; Linux is kept honest
  by CI but isn't daily-driven. X11/Wayland bugs, packaging, and the tray GTK
  loop are especially welcome.
- **Cross-platform correctness** — anything that keeps the same code working on
  both macOS and Linux.
- **Docs** — clarifications to `README.md`, `docs/LEARNINGS.md`, or
  `docs/UPGRADING.md`; hard-won GPUI gotchas are valuable.
- **Small, focused features** that stay true to "kept small" — the starter is
  ~700 lines on purpose.

Probably out of scope: large frameworks, your app's domain logic, or anything
that bloats the starter. When in doubt, open an issue first.

## The rules (single-sourced in `CLAUDE.md` / `AGENTS.md`)

The contributor rules and the agent rules are the same rules. The authoritative
copy — including the full **Definition of Done**, the code constraints, and the
performance rules — lives in **[`CLAUDE.md`](CLAUDE.md)** (mirrored verbatim in
[`AGENTS.md`](AGENTS.md)). The *why* behind every lint and gate is in
[`docs/AGENTIC-ENGINEERING.md`](docs/AGENTIC-ENGINEERING.md). Read those before a
non-trivial change; the essentials are restated below.

## The loop

Install [`just`](https://github.com/casey/just) (`brew install just`), then:

```bash
just run     # run the app (default features)
just fix     # auto-apply clippy's machine-fixable suggestions + format
just ci      # the FULL Definition of Done in one command (run before you push)
```

`just ci` mirrors `.github/workflows/ci.yml`, so **green locally == green in
CI.** Loop `just fix` → `just ci` to self-correct without a CI round-trip.

## Definition of Done

A change is done only when all of these hold — `just ci` checks them at once.
**Don't open a PR while anything is red.**

1. `cargo fmt --all --check` is clean.
2. `clippy -D warnings` is green on **every** feature config: default, `tray`,
   `overlay`, and `tray,overlay` (`just check`).
3. `cargo test` is green — including `cargo test --features overlay` (the
   `command_palette` fuzzy-match tests and the overlay reducers live here).
4. **No new or changed dependencies** in `Cargo.toml` / `Cargo.lock` without
   explicit maintainer approval — open an issue to discuss first. A new store or
   network client is a new dep and is approval-gated.

### Bumping GPUI

The `gpui` / `gpui_platform` / `gpui-component` / `gpui-component-assets` stack
is pinned to exact commits in `Cargo.lock`. **Never hand-edit those pins.** Bump
them only with:

```bash
just bump-gpui   # cargo update on the four crates + rebuild
```

then commit the updated `Cargo.lock` (and `rust-toolchain.toml` if the build
needed a newer toolchain). Full procedure and the crates.io fallback channel:
[`docs/UPGRADING.md`](docs/UPGRADING.md).

## Submitting a PR

1. Branch off `main`.
2. Make the change; run `just ci` until green.
3. Open a PR and fill in the template — paste the `just ci` output as evidence
   and confirm the no-new-deps box.

Contributions are accepted under the project's [`0BSD`](LICENSE) license.

## Reporting bugs & security

- **Bugs / features:** use the issue templates (Bug report / Feature request).
- **Security vulnerabilities:** do **not** open a public issue — see
  [`SECURITY.md`](SECURITY.md).
