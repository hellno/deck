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
  hard-won GPUI gotchas are valuable.
- **Small, focused features** that stay true to "kept small" — the starter is
  small on purpose.

Probably out of scope: large frameworks, your app's domain logic, or anything
that bloats the starter. When in doubt, open an issue first.

## The rules

The contributor rules and the agent rules are the same: [`AGENTS.md`](AGENTS.md) (template
mechanics, Definition of Done) and the generated app's guide, `AGENTS.md.liquid`. The *why*
behind each lint and gate is in [`docs/AGENTIC-ENGINEERING.md`](docs/AGENTIC-ENGINEERING.md).

## The loop

This repo is a cargo-generate template and does not build directly. Install
[`just`](https://github.com/casey/just) and `cargo-generate`, then:

```bash
just check-template                     # render a project and run its `just ci`
just shot settings dark /tmp/s.png      # headless screenshot of the rendered app
```

New files must be staged (`git add`) before `just check-template` sees them.

## Definition of Done

1. `just check-template` is green: fmt, clippy `-D warnings` on default and
   `--all-features`, tests, and a binary build.
2. UI changes: `just shot` in light and dark, attached to the PR.
3. **No new or changed dependencies** without maintainer approval; open an issue first.
4. Upgrading GPUI: bump `gpui-kit` in `Cargo.toml`, then `just update-lock -p gpui-kit`
   (see `AGENTS.md`).

## Commits

`type(scope): summary` (`fix(palette): …`, `docs: …`), then why, with the commands behind
it. Docs describe what exists now; history lives in git.

## Submitting a PR

1. Branch off `main`.
2. Make the change; run `just check-template` until green.
3. Open a PR and fill in the template — paste the `just check-template` output as evidence
   and confirm the no-new-deps box.

Contributions are accepted under the project's [`0BSD`](LICENSE) license.

## Reporting bugs & security

- **Bugs / features:** use the issue templates (Bug report / Feature request).
- **Security vulnerabilities:** do **not** open a public issue — see
  [`SECURITY.md`](SECURITY.md).
