# Agentic engineering: lints, constraints & CI for building fast with coding agents

> **Audience:** anyone who forks **Deck** to build their own native GPUI app, and wants a coding
> agent (or a hurried human) to ship without quietly breaking things. This is the *why* behind
> Deck's lint/CI config; the *what to run* lives in `CLAUDE.md` / `AGENTS.md`. Deck is a
> **single-crate** starter (no `[workspace]`), so everything here is package-level — there is no
> `[workspace.lints]` indirection to set up.

## The thesis

A coding agent is fast but has no taste and no memory of *this* repo's intent. The cheapest way to
make an agent reliably produce good code is to **make the compiler and CI say no for you.** Every
rule below converts a class of mistake from "caught in review, maybe" into "caught at `cargo check`,
always."

Three principles, in priority order:

1. **The manifest is the source of truth, not CI flags.** If the lint policy only lives in a CI
   `-D warnings` flag, the agent doesn't see it until the build is already red. Put it in
   `Cargo.toml` (`[lints.rust]` / `[lints.clippy]`) and `clippy.toml` so `rust-analyzer` and
   `cargo check` show the *exact same* policy at the moment code is written. **Shorten the feedback
   loop to zero.**
2. **CI is the only gate that matters.** Pre-commit hooks, editor warnings, and good intentions are
   all bypassable (`git commit --no-verify`, "I'll fix it later"). An agent will confidently report
   "done" while red. So anything you actually care about must *block merge* in CI.
3. **Prefer compile-time over runtime, and `warn` → fix → `deny` over big-bang.** Land a new lint as
   `warn`, clear the backlog, *then* flip to `deny`. A rule that breaks `main` on day one gets
   reverted; a rule that lands green stays forever.

These match what the paradigm Rust projects do — the source research cross-checked
[reth](https://github.com/paradigmxyz/reth), [alloy](https://github.com/alloy-rs/alloy),
[Zed](https://github.com/zed-industries/zed) (Deck's GPUI source), tokio, ripgrep, and the
[Embark Studios shared lint set](https://github.com/EmbarkStudios/rust-ecosystem/blob/main/lints.rs).
The recurring pattern is identical: a lint table in the manifest, a `clippy.toml`, a checked-in
`rustfmt.toml`, `cargo-deny` in CI, and a CI matrix that runs fmt + clippy + test on every push.

---

## Tier 1 — the do-now set

Each item: **what**, **why it helps an agent**, the **snippet**, and the **tradeoff**.

### 1. Put the lint policy in the manifest — `[lints]` in `Cargo.toml`

**What.** Deck is a single crate, so the lint policy goes **directly** in `Cargo.toml` under
`[lints.rust]` and `[lints.clippy]`. (There is no `[workspace.lints]` table and no
`[lints] workspace = true` line — that two-table indirection is only for multi-crate *workspaces*.
For a single crate it would be wrong.) Today Deck's `-D warnings` policy exists *only* as a CLI flag
in `just check` and CI.

```toml
# Cargo.toml — directly under the package, single-crate style
[lints.rust]
unused_must_use = "deny"   # an ignored Result from a fallible IO/serde call is a silent bug
unsafe_code     = "deny"   # deny (not forbid) — see §6 for why the escape hatch matters

[lints.clippy]
all       = { level = "warn", priority = -1 }  # priority = -1 is required on a lint *group*
todo      = "deny"          # a stray todo!() left on a code path panics in production
dbg_macro = "deny"          # dbg!(x) debug noise left behind by an agent
```

**Why for an agent.** The agent now sees `clippy::all` and the deny-list in-editor and at
`cargo check`, identical to what CI enforces — no more "looked fine locally, red in CI." `todo` /
`dbg_macro` at `deny` make two classic agent habits (leaving a `todo!()` stub, leaving a `dbg!`)
into hard compile errors. All four deny-level items are pre-verified clean against Deck's `src/`
today (no `todo!`/`dbg!`/`unsafe` anywhere, and no ignored `Result`s), so this lands green.

**Tradeoff.** `clippy::all` stays at `warn` in the manifest (not `deny`) while CI keeps
`-D warnings`. That's deliberate: `warn` lets you iterate locally without every style nit blocking
`cargo build`, while CI still fails on any warning. Setting `clippy::all = "deny"` would make a
*future toolchain bump* (new clippy lints) break local builds for unrelated reasons. `warn` + CI
gate is the reth/alloy convention.

### 2. A project lint config — `clippy.toml`

**What.** New file at the repo root.

```toml
msrv = "1.95.0"                 # match rust-toolchain.toml; governs which API suggestions clippy makes
allow-unwrap-in-tests = true    # the command_palette fuzzy-match tests unwrap() freely
allow-expect-in-tests = true
allow-panic-in-tests  = true

# Deck only enables clippy::all, which does NOT include doc_markdown, so doc-valid-idents is
# unnecessary today. If you later turn on clippy::pedantic, add your own domain jargon here so
# doc_markdown stops flagging it, e.g.:
#   doc-valid-idents = ["..", "GPUI", "wgpu", "macOS"]   # ".." keeps clippy's defaults

# disallowed-methods is the highest-signal clippy tool: it turns "don't call X here" tribal
# knowledge into a compile error WITH the reason printed. Deck ships none by default; ban your
# own footguns as you discover them, e.g.:
#   disallowed-methods = [
#     { path = "std::process::exit", reason = "return from main / propagate an error instead" },
#   ]
```

**Why for an agent.** `disallowed-methods` is the highest-signal-per-line tool clippy offers — it
prints *your* reason at `cargo check`. It's left as a commented example so a forker sees the
mechanism and can ban whatever their app must never call. The `allow-*-in-tests` keys keep Deck's
existing `command_palette` tests (which `unwrap()` scores freely) legal even if you later add
stricter crate-level restriction lints.

**Tradeoff.** None material — the only active key is `msrv`; everything else is opt-in scaffolding.

### 3. Deterministic formatting — `rustfmt.toml`

**What.** New file at the repo root. All keys are **stable-channel** (Deck's pinned 1.95.0 stable
`cargo fmt` honors them; nightly-only keys like `imports_granularity` / `group_imports` are
deliberately excluded — stable silently ignores them, which is worse than not setting them).

```toml
edition = "2021"
max_width = 100
```

Intentionally minimal — `edition` + the width the code is already written to (both rustfmt defaults
today), so landing it is near-zero churn. Style idioms are left to `clippy::all`.

**Why for an agent.** Without a checked-in config, two agents (or two rustfmt versions) format the
same code differently, producing noisy diffs that bury the real change. A pinned config makes the
diff deterministic, and `cargo fmt --all --check` in CI (§5) makes "did you run fmt?" a yes/no gate
instead of a review comment.

**Tradeoff.** None for Deck: the repo is already `cargo fmt --check`-clean, so enabling the
`--check` gate needs **no** baseline-format commit (unlike a repo that has to reformat first).

### 4. Supply-chain gate — `deny.toml` + a `cargo-deny` CI job

**What.** `cargo-deny` checks the dependency tree for security advisories, banned/duplicate crates,
disallowed licenses, and untrusted sources. New `deny.toml`:

```toml
[advisories]
yanked = "deny"
ignore = []   # add { id = "RUSTSEC-…", reason = "…" } only with written justification
# (cargo-deny v2 denies vulnerability advisories and unmaintained crates by default.)

[licenses]
version = 2
confidence-threshold = 0.9
# SEED-THEN-TIGHTEN: run `cargo deny check licenses` locally and reconcile this list before
# making the CI job blocking — the git gpui stack pulls a wide license surface and a blind list
# WILL red the build. Deck's OWN crate is 0BSD (cargo-deny license-checks first-party crates too),
# which is permissive, so it's just another entry in the allow list — no exceptions block needed.
allow = ["MIT", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause",
         "0BSD", "ISC", "Unicode-3.0", "Zlib", "MPL-2.0", "Unlicense", "CC0-1.0"]

[bans]
multiple-versions = "warn"   # the git gpui stack legitimately duplicates crates (objc2 0.5+0.6) — deny is permanently red
wildcards = "deny"
deny = [{ crate = "openssl", reason = "prefer rustls/ring; avoid the OpenSSL CVE surface" }]

[sources]
unknown-registry = "deny"
unknown-git = "deny"
# All SIX git origins, not two: Zed pins its own forks of font-kit/reqwest/scap/wgpu, pulled
# transitively by the gpui stack. These are the exact forms recorded in Cargo.lock (note the
# `.git` suffix on reqwest/wgpu). Re-derive with `grep 'git+' Cargo.lock` after every
# `just bump-gpui` — Zed may add or retire forks.
allow-git = ["https://github.com/zed-industries/zed",
             "https://github.com/zed-industries/font-kit",
             "https://github.com/zed-industries/reqwest.git",
             "https://github.com/zed-industries/scap",
             "https://github.com/zed-industries/wgpu.git",
             "https://github.com/longbridge/gpui-component"]
```

**Why for an agent.** Three failure modes, gated at once: (a) pulling in a crate with a known
RUSTSEC advisory, (b) adding a dep from a random git fork — the `[sources]` allow-list permits only
the known origins (the six Zed/longbridge ones the gpui stack pulls), so a typo-squat or malicious
fork fails CI, (c) license drift. Cheap insurance for a project people fork and ship.

**Tradeoff.** The license `allow` list must be **seeded locally first** (`cargo deny check
licenses`), and `multiple-versions` must stay `warn` because the git gpui tree legitimately
duplicates crates. So the CI job lands **non-blocking** and is promoted after one green run (see
Rollout).

### 5. Close the CI gaps

**What.** Deck's CI currently runs only `cargo build`, `cargo build --features tray`, and
`cargo clippy --all-targets --features tray -- -D warnings`. That means: **format is never checked,
tests never run, and clippy never lints the default (`--features`-less) build that Deck actually
ships.** That last one matters most for agents: there are 6 real `#[test]` fns in
`src/command_palette.rs` (fuzzy-match scoring) that **CI never runs today** — an agent can break
the scoring and CI stays green. Add (split across the existing macOS/Linux jobs to respect the
macOS minute multiplier):

```yaml
# linux job (formatting is OS-independent, so check it once on the cheap runner):
- run: cargo fmt --all --check
- run: cargo clippy --locked --all-targets -- -D warnings   # the DEFAULT feature config CI never linted
- run: cargo test --locked

# macOS job:
- run: cargo clippy --locked --all-targets -- -D warnings
- run: cargo test --locked
```

> Add `--locked` to **every** `cargo build`/`clippy`/`test` invocation: reproducibility lives
> entirely in the committed `Cargo.lock` (it pins the exact git gpui commits), so `--locked` makes a
> stale lockfile a CI failure. `just bump-gpui` rewrites and commits the lock, so this never fights
> the bump workflow.

Plus the `cargo-deny` job (non-blocking first):

```yaml
cargo-deny:
  runs-on: ubuntu-latest
  # Informational on first land. Promote to a required check after one green run AND after seeding
  # deny.toml [licenses].allow from a local `cargo deny check licenses`.
  continue-on-error: true
  steps:
    - uses: actions/checkout@v4
    - uses: EmbarkStudios/cargo-deny-action@v2
      with: { command: check advisories bans sources licenses }
```

**Why for an agent.** This is principle #2 made real. Running `cargo test` and the default-feature
clippy closes the "green CI but actually broken" hole that lets an agent honestly believe it's done.

**Tradeoff.** Slightly longer CI; the macOS runner is 10× billed on *private* repos (Deck is
public, so free) — hence formatting and `cargo-deny` run only on Linux. Preserve the existing CI
structure when adding these: the macOS cost note, the Linux system-deps apt block, the
rust-toolchain-as-source-of-truth note, `Swatinem/rust-cache`, and `env: CARGO_TERM_COLOR`.

### 6. Unsafe policy — deny, with a reviewed escape hatch

**What.**

```toml
# Cargo.toml [lints.rust]
unsafe_code = "deny"      # deny, NOT forbid — see below
```

**Why for an agent.** Deck has **zero** `unsafe` in `src/` today. The only Apple-FFI in the whole
tree is the tray feature's dock-hiding via `objc2`, and it goes through *safe* `objc2` wrappers — so
`unsafe_code = "deny"` compiles today for **both** the default and `--features tray` builds. Denying
it means no agent can quietly introduce a raw `unsafe {}` block without it showing up as a compile
error.

**Tradeoff — why `deny` not `forbid`:** `forbid` cannot be locally overridden. If a future `objc2`
bump reintroduces a raw `unsafe {}` block in the tray path, `forbid` would brick the build with no
escape hatch. `deny` lets you add a single, reviewed, `// SAFETY:`-commented `#[allow(unsafe_code)]`
at that exact site. (Note: `[lints]` governs first-party code only — `unsafe` inside dependencies is
unaffected, which is correct.)

### 7. Tell the agent the rules — `CLAUDE.md` + `AGENTS.md`

**What.** Deck has neither file today; create both. `CLAUDE.md` is what Claude Code reads;
`AGENTS.md` is what Codex and other agents read. Keep them in lockstep and point both here for the
rationale. The essentials: the fast iteration command, an explicit **definition of done**, and the
code constraints (in prose, mirroring the lints).

```markdown
## Engineering & verification
Iterate fast: `cargo check` for a quick type-check loop. The full app build pulls gpui from git, so
the FIRST build is slow (cached after).
Definition of done (ALL must hold; paste the command output as evidence — never claim done while red):
1. `cargo fmt --all --check` clean
2. `just check` green — clippy `-D warnings` on BOTH the default and `--features tray` configs
3. `cargo test` green
4. No new/changed deps (Cargo.toml or Cargo.lock) unless explicitly approved; bump the git gpui
   stack ONLY via `just bump-gpui`, never hand-edit those pins.

## Code constraints
`todo!`/`dbg!` denied; ignored Results (`unused_must_use`) denied; `unsafe_code` denied (a new
unsafe block needs a reviewed `// SAFETY:` comment + a scoped `#[allow(unsafe_code)]`). Deck is an
app, so `.expect()` on genuinely-infallible GPUI handles is fine (see `main.rs` / `tray.rs`) —
prefer `Result`/`?` everywhere else.
```

**Why for an agent.** An explicit "definition of done with evidence" is the single most effective
guardrail against an agent declaring victory while red. The constraints duplicate what the lints
enforce, in prose, so the agent internalizes them *before* writing rather than learning from a
failed build.

**Tradeoff.** Two files to keep in sync. Keep them short (~40–55 lines) and point both here.

> **Design pointer for UI work:** ground changes in `README.md` and `docs/LEARNINGS.md` (real
> screenshots and hard-won GPUI lessons), not in remembered descriptions of how GPUI behaves.

### 8. Close the local OODA loop — `just ci` / `just fix`, editor == CI, parseable diagnostics

**What.** Everything above gives the agent *rules*; this gives it the *loop* — the ability to
observe a verdict and self-correct without a CI round-trip. Five small pieces:

```makefile
# justfile — one command that IS the Definition of Done, plus one-shot auto-remediation
ci:
    cargo fmt --all --check
    cargo clippy --locked --all-targets -- -D warnings
    cargo clippy --locked --all-targets --features tray -- -D warnings
    cargo test --locked
fix:
    cargo clippy --fix --allow-dirty --allow-staged --all-targets
    cargo clippy --fix --allow-dirty --allow-staged --all-targets --features tray
    cargo fmt
```

- **`just ci`** runs the exact CI gate locally. Before this, `just check` ran clippy *only* (no
  fmt-check, no tests) — so an agent that ran it, saw green, and reported "done" was still red in CI
  on formatting or a broken test. One command now collapses the whole Observe step into a single
  pass/fail.
- **`just fix`** auto-applies clippy's machine-fixable suggestions and reformats — the correct Act
  when `just ci` reports a fixable nit. The agent loops `fix` → `ci` instead of hand-editing.
- **`.vscode/settings.json` + `.zed/settings.json`** set rust-analyzer's check command to `clippy`.
  This is the precondition for principle #1: rust-analyzer defaults to plain `cargo check`, which
  does **not** run the `[lints.clippy]` rules — so without this the in-editor signal would silently
  diverge from CI. (Deck is itself a Zed app, so the `.zed/` config is doubly apt.)
- **`--message-format=short` / `=json`** — tell the agent (in `CLAUDE.md`/`AGENTS.md`) to read
  diagnostics in machine-readable form. `short` is one line per diagnostic; `json` carries
  structured spans and applicable fixes. Far more robust for an agent than scraping rendered
  carets/ANSI.
- **`.editorconfig` + CI `concurrency` cancel** — deterministic whitespace for the files rustfmt
  doesn't touch (YAML/TOML/Markdown), and cancellation of superseded CI runs so a fix-up push gets
  a verdict on the *latest* commit instead of queueing behind a stale slow build.

**Why for an agent.** This is the difference between an agent that needs a human (or a 15-minute CI
round-trip) to learn it's wrong, and one that runs `just ci`, reads structured failures, runs
`just fix`, and converges on its own. It is the most direct lever on "run longer, unattended."

**Tradeoff.** A few lines of `justfile` + two tiny editor configs; `just ci` duplicates the CI step
list, so keep the two in sync (they're short). All of it is zero new dependencies.

---

## Deliberately NOT recommended (so we don't over-rotate)

| Tempting | Why we skip it |
|---|---|
| `#![forbid(unsafe_code)]` at the crate root | No escape hatch if an `objc2` bump needs a raw `unsafe` in the tray path. Use `unsafe_code = "deny"` in the manifest instead (§6). |
| `clippy::pedantic` / `clippy::nursery` globally | Too noisy for a small UI app; nursery lints are unstable → a toolchain bump can surface new warnings and break `-D warnings`. Cherry-pick instead. |
| The full `clippy::restriction` group | Clippy's own docs say never enable it wholesale (it contains mutually contradictory lints). Pick individual ones. |
| Edition 2024 bump | Deck pins 1.95.0 in lockstep with gpui's git HEAD; an edition bump is orthogonal churn that risks the gpui pairing. |
| Nightly rustfmt keys / nightly build flags (`-Zthreads`, cranelift, `-Zshare-generics`) | The 1.95.0 stable pin is **mandatory** for the git gpui build — nightly flags in a committed config brick `cargo build` for everyone. |
| `panic = "abort"` | Deck's `command_palette` tests rely on unwinding; `abort` also loses the backtrace on a panic. Keep the default unwinding. |
| `multiple-versions = "deny"` in deny.toml | The git gpui stack pulls duplicate versions (objc2 0.5+0.6); permanently red. Keep `warn`. |
| mold/lld linker config | Not worth the per-machine setup churn for a starter: it forces every forker/CI runner to install and configure an alternate linker, and on macOS `lld` is a measured *regression* vs Apple's default `ld`/`ld-prime`. The stock toolchain links fine. Do nothing. |
| deckard-core-style crate-level `#![deny(clippy::unwrap_used, expect_used, panic, indexing_slicing)]` + `#![forbid(unsafe_code)]` on a security core | Deck has **no** security/crypto core — that hardening is wallet-specific and intentionally not applied here (e.g. `expect_used` would red `src/main.rs` and `src/tray.rs`, where expecting an infallible GPUI handle is correct). If you fork Deck into something security-sensitive, add a headless, GPUI-free core crate and adopt that hardening there. See the [deckard PR](https://github.com/hellno/deckard/pull/8) for the full pattern (bounds-checked reader, `Zeroizing` secrets, `#[must_use]` on secret types). |

---

## Rollout order (stays green at every step)

1. **Formatting.** Add `rustfmt.toml`. Deck is already fmt-clean, so `--check` passes immediately —
   no baseline-format commit needed.
2. **Lints.** Add `[lints.rust]` / `[lints.clippy]` to `Cargo.toml` + `clippy.toml`. Run clippy
   (both feature configs); the deny-level items are pre-verified clean.
3. **Unsafe policy.** `unsafe_code = "deny"` in the manifest (compiles for default and `--features tray`).
4. **CI gaps.** Add fmt-check, default-feature clippy, and `cargo test` (with `--locked`). Land,
   confirm green, mark required.
5. **deny.toml.** Seed `[licenses].allow` from a local `cargo deny check licenses` run *first*; land
   the `cargo-deny` job non-blocking; promote to required after one green run.
6. **Docs.** Add `CLAUDE.md` and `AGENTS.md`.
7. **Local loop.** Add the `just ci` / `just fix` recipes, `.vscode//.zed/` clippy-on-save,
   `.editorconfig`, and the CI `concurrency` cancel. All green by construction (no code change).

The invariant: **every new lint enters as `warn`; every new CI job enters non-blocking. Confirm
green, then tighten.** `main` is never red because of a hardening change.

---

## Provenance

Adapted for Deck from the [deckard PR #8](https://github.com/hellno/deckard/pull/8) multi-agent
research sweep of paradigm Rust OSS repos (reth, alloy, Zed, tokio, ripgrep, Embark), plus a
feasibility pass that verified each recommendation against real source. The wallet-specific rules
from that work (a `#![forbid(unsafe_code)]` security core, `unwrap_used`/`expect_used`/`panic`/
`indexing_slicing` denies, `Zeroizing` secrets, the bounds-checked reader, crypto `doc-valid-idents`,
and `mem::forget`/`thread_rng` `disallowed-methods`) are deliberately **dropped** here — Deck is a
fork-it-rename-it GPUI starter, not a security-sensitive app. What remains is the general-purpose
core that applies to any GPUI + Rust project.
