# Spec: Floating overlay surfaces for Deck

Status: DESIGN (reviewed) · Scope: macOS-only v1 · Audience: reusable Deck template flow
Branch: `hellno/hover-overlay-window` · Research backing: `.context/floating-overlay-design.md` (working notes, uncommitted)
Decision log: #4 (over-other-apps hardening) is a **v1 gate**; its P2 spike runs first.
Independent review: codex (gpt) scored the v1 draft **5/10** executability; this revision folds in its
ten findings (status-push root-type bug, P2 spike, resize wording, lifecycle, Linux/CI, dead-zone, objc2
helper, unsafe/dep wording). Corrections are marked `[codex]` inline.

> ## Redesign (2026-06-10) — two small transparent surfaces, no `Root` wrapper
>
> The original v1 below shipped **two oversized opaque dark rectangles** out of one
> 460×160 window (a top-right status row + a bottom Wispr-style toolbar/capsule/banner
> state machine). User feedback: too big, too many buttons, no hover, too much for a
> mock. The redesign replaces both with **two small, genuinely-transparent surfaces
> rendered together** — each its own content-sized transparent `PopUp` window:
>
> - **Top-right RAIL** (`src/overlay/rail.rs`) — a small frosted vertical panel: generic
>   job-status icons → a thin divider → **3 square clickable action buttons** (basic
>   hover). ≈76×300.
> - **Bottom-center PILL** (`src/overlay/pill.rs`) — a small frosted pill with a circular
>   **record button** that pulses red while recording; toggled by click and by `space`
>   while Deck is focused. ≈220×64.
>
> **Root cause of the old "dark box," now fixed:** the overlay wrapped its view in
> gpui-component `Root`, and `Root::render` paints an opaque `theme.background` over the
> whole window. The redesign drops `Root` — each window's `open_window` build closure
> returns the view entity directly — so the window stays transparent and only the frosted
> panels paint. The simplified surfaces need none of `Root`'s layers (no
> tooltips/notifications/modals), so skipping it costs nothing. See CHILD #2 / #3 below.
>
> **What was removed as over-built:** the single shared 460×160 canvas, the
> `HudState` dormant→toolbar→capsule→banner machine + `Transition` morphs, tooltips, the
> inline `Kbd` chips, and the `OverlayState` opacity-pulse show/hide workaround. The
> `overlay_anchor` user setting + `DECK_OVERLAY_ANCHOR` env override are gone too —
> `OverlayAnchor` survives only as an **internal positioning helper** in `state.rs`
> (rail = `TopRight`, pill = `BottomCenter`); `DECK_OVERLAY=0/1` remains the master on/off.
> The two `unsafe` blocks in `harden.rs` are unchanged; no new deps.
>
> The EPIC, CHILD #1, §3/§4 spike history, and the §4 SPIKE RESULT below are retained as
> accurate **history** (the transparent-PopUp primitive, the P2 focus spike, and the
> objc2 bridge all carry forward). Where they describe the *old* surfaces (one window,
> the HUD state machine, the anchor picker) as the **current** design, they are annotated
> or superseded by the redesign notes inline.

All API claims verified against the pinned sources:
- GPUI `~/.cargo/git/checkouts/zed-a70e2ad075855582/86effff/crates/{gpui,gpui_macos}`
- gpui-component `~/.cargo/git/checkouts/gpui-component-95ce574d8a0da8b8/dadfca9`
- Deck: this repo.

---

## EPIC — Floating overlay surfaces (always-on-top transparent HUD primitive + agent-row + Wispr pill)

### Context
Deck opens exactly one normal window (`src/main.rs:134`). There is no way to show ambient, always-on-top status — the thing every async-agent tool (Wispr Flow, Raycast, Linear) leans on. This epic adds a **reusable overlay surface** to the starter: a transparent, borderless, always-on-top window (`WindowKind::PopUp`) you mount glanceable status/controls into, gated behind `--features overlay` so the default fork stays lean. The visual/foundation tiers need **no new crates and no app-level unsafe**; the over-other-apps interaction hardening is fenced into its own child (#4) and may carry one scoped `unsafe`. macOS-only for v1; a `#[cfg(target_os)]` seam is left for a later Linux/Wayland `LayerShell` pass but is **not** a v1 acceptance gate.

### Child issues
| # | Title | Priority | Effort | Depends on | New crate / unsafe |
|---|-------|----------|--------|-----------|--------------------|
| 1 | Overlay **primitive** (`--features overlay`, transparent PopUp, edge-anchored, status-push spine, lifecycle) | Critical | ~2d | — | none / none |
| 2 | **Top-right rail** surface (generic job-status icons + divider + 3 square action buttons; no agent-specific UI) `[redesign]` | High | ~1d | #1 | none / none |
| 3 | **Bottom-center recording pill** surface (record button + `space`-while-focused toggle + red pulse) `[redesign]` | High | ~1d | #1 | none / none |
| 4 | **macOS interaction hardening** (no-steal-on-click + click-through) — **v1 GATE** for #3; the surfaces must work over other apps | Critical | spike + ~1.5d | #1 | none / likely 1 scoped unsafe |

> `[redesign 2026-06-10]` #2 is now the top-right **rail** and #3 the bottom-center
> **recording pill** (two separate transparent windows, no `Root`). The old #3 HUD state
> machine (dormant→toolbar→recording→banner) is removed. See the redesign note above and
> the rewritten CHILD #2 / #3 sections.

### Dependency graph
```
#1 Overlay primitive ──> #4 SPIKE (P2: prove no-steal) ─┬─> #2 Status-icon row
                                                        └─> #3 Wispr HUD pill ──> #4 rest (P3 + harden)
```
### Sequencing rationale
#1 is the shared window/anchoring/status-push/lifecycle foundation. **Because #4 is a v1 gate and its P2 mechanism is unproven** (`becomesKeyOnlyIfNeeded` on GPUI's single NSView), its spike runs **immediately after #1** — de-risk the over-other-apps requirement before investing in #3's full surface. If the spike fails, we learn it cheaply and renegotiate scope. #2 is still the cheapest *visual* proof of the animation + status-push spine (no focus caveats), so it proceeds in parallel once #1 lands; #3 follows, gated on #4's hardening.

### Definition of Done (epic)
1. `cargo run --features overlay` shows a transparent always-on-top window with at least one live surface.
2. A background task pushes a state change into the overlay and it repaints (no UI-thread I/O).
3. **Over-other-apps (v1 gate, #4):** with another app foreground, clicking a HUD button does not move that app's text-field focus (P2), and clicks on transparent overlay pixels reach the app below (P3, per the chosen option).
4. Closing the main window tears the overlay down cleanly (no leaked task/Global); pushes after close are no-ops. `[codex #8]`
5. `just ci` green on default, `--features tray`, **and** `--features overlay` (+ combined `tray,overlay`), on macOS **and** Linux (Linux `overlay` compiles to a no-op). `[codex #9]`
6. Default `cargo run` is unchanged (feature additive, off by default).

### Out of scope (epic)
Cursor-trail and screen-draw surfaces; global hotkeys; AX focused-textbox detection; text insertion into other apps; Linux/Wayland `LayerShell` impl; real audio/dictation. Documented Phase-3 follow-ups.

### Rollback
Pure-additive + feature-gated: revert the PR or drop `--features overlay`. No migration, no shared-state change.

---

## CHILD #1 — Overlay primitive (`--features overlay`)

### Context
The foundation: open a second transparent, borderless, always-on-top window anchored to the active display, expose an `Entity<Overlay>` a background task can push into, manage its lifecycle, and gate it behind `--features overlay` mirroring `--features tray`.

### Implementation details
- **Module layout:** `src/overlay/mod.rs` (`pub fn install(cx, &Settings)`), `src/overlay/state.rs` (`OverlayState`, `OverlayAnchor` enums + transitions; unit-tested like `command_palette.rs:217`/`:754`), `src/overlay/view.rs` (`struct Overlay` + `impl Render`).
  **`[redesign 2026-06-10]`** After the redesign the module layout is: `mod.rs` (opens two
  `Root`-less content-sized windows + the `ToggleRecording` action/`space` binding + weak
  handles), `rail.rs` (top-right rail, **new**), `pill.rs` (recording pill, was `hud.rs`),
  `status.rs` (pure `JobStatus` logic, was `status_row.rs`), `state.rs` (`OverlayAnchor`
  positioning helper only — `OverlayState` removed), `harden.rs` (unchanged). `view.rs` is
  deleted.
- **macOS-only window, Linux no-op `[codex #9]`:** put the window-opening body behind `#[cfg(target_os = "macos")]`; the non-macOS `install()` is an empty no-op. objc2 deps stay macOS-only (they already are: `Cargo.toml:65-68`). This keeps Linux `cargo clippy --features overlay` compiling green without a LayerShell impl.
- **Window:** `cx.open_window` (`gpui/src/app.rs:1136`). Inside the closure, build the overlay view first, then wrap it:
  ```rust
  let opts = WindowOptions {
      kind: WindowKind::PopUp,            // non-activating NSPanel + NSPopUpWindowLevel(101) + all-Spaces
      titlebar: None,                     // borderless
      focus: false,                       // don't grab focus on open
      window_background: WindowBackgroundAppearance::Transparent,
      is_movable: false, is_resizable: false, is_minimizable: false,
      display_id: Some(display_id),
      window_bounds: Some(WindowBounds::Windowed(bounds)), // sized once to MAX footprint
      ..Default::default()
  };
  let handle: WindowHandle<Root> = cx.open_window(opts, |window, cx| {
      let overlay = cx.new(|cx| Overlay::new(anchor, window, cx));   // Entity<Overlay>
      cx.set_global(OverlayHandle { overlay: overlay.downgrade(), window: window.window_handle() });
      cx.new(|cx| Root::new(overlay.clone().into(), window, cx))     // Root wraps the view
  })?;
  ```
  `PopUp` already yields non-activating panel + popup level + all-Spaces (`gpui_macos/src/window.rs:714,919,924-927`). Root wrap is required (`main.rs:135-137`) or tooltips/notifications no-op.
  **`[redesign 2026-06-10]`** This is the precise trap the redesign fixes: `Root::render`
  paints an opaque `theme.background` over the whole window (the dark box). `Root` is
  required **only if you need its tooltip / notification / modal layers** — `YES`
  permits them, it does not require them. The redesigned rail/pill use none of those
  layers, so they **return the view entity directly** (no `Root`) and the window stays
  transparent. Wrap in `Root` only when a surface actually needs those overlay layers.
- **Status-push spine `[codex #1 — the must-fix bug]`:** `cx.open_window` returns `WindowHandle<Root>`, NOT `WindowHandle<Overlay>` — the root is gpui-component `Root` (`root.rs:30`, it holds `view: AnyView`). So you **cannot** do `handle.update(|o| o.state = ...)`; that gives `&mut Root`. Instead stash a **`WeakEntity<Overlay>`** in a GPUI `Global` (`OverlayHandle`, mirroring `tray.rs:29-33` `TrayState`) and push through the *entity*:
  ```rust
  // background work then hop back to the main thread:
  cx.background_executor().spawn(heavy).await;                 // app.rs:1714
  cx.update(|cx| {                                             // AsyncApp::update, app.rs:1729
      if let Some(overlay) = cx.global::<OverlayHandle>().overlay.upgrade() {
          overlay.update(cx, |o, cx| { o.state = next; cx.notify(); });
      }
  })?;                                                          // handle the Result (unused_must_use)
  ```
  `WeakEntity::upgrade()` returning `None` is the natural no-op-after-close. `[codex #8]` This is the generic
  background-job spine — see `docs/background-jobs.md` for the cancellation/retry/HTTP-client pattern the agent-row (#2) reuses.
- **Lifecycle `[codex #8]`:** subscribe to the main window's close (or `cx.on_window_closed`) and close the overlay with it; on overlay close, remove `OverlayHandle` from globals so later pushes find nothing and no-op; ensure the demo background task observes the weak handle and exits. Decide explicitly whether `cx.quit()` should fire when only the overlay remains (recommend: overlay never keeps the app alive — quit when the main window closes).
- **Anchoring + the resize question `[codex #3]`:** GPUI *does* expose `Window::resize()` (`window.rs:2217`), but it has no origin/bounds setter (`PlatformWindow`, `platform.rs:614-660`), so a resize anchored bottom-center would drift. v1 **chooses the fixed-canvas approach**: size the window once to the max footprint and animate a child inside it. Document the chosen **dead-zone budget** (see #4/P3). Compute the anchor rect from `cx.primary_display()`/`displays()` (`app.rs:1192,1197`) + `PlatformDisplay::visible_bounds()` (`platform.rs:262`).
- **Settings:** add `#[serde(default)]` `overlay_enabled: bool` + `overlay_anchor: OverlayAnchor` to `Settings` (`settings.rs:43-61`; update `Default` `:52-61`). Persist via `save_best_effort()` (`settings.rs:108`) at a commit boundary only. **`[redesign 2026-06-10]`** `overlay_anchor` is **removed** — only `overlay_enabled` remains a setting (see §5).
- **Wiring:** `#[cfg(feature="overlay")] mod overlay;` (`main.rs:15-16`) + `#[cfg(feature="overlay")] overlay::install(cx, &settings);` after the main `open_window` (`main.rs:142-143` pattern). Capture overlay fields before `settings` moves into the Shell closure (mirror `main.rs:60`). **Do not** call `cx.activate(true)` when summoning the overlay — that re-activates Deck and re-introduces P1. `[codex #6]`
- **Cargo/CI:** `overlay = []` in `[features]` (`Cargo.toml:60`). Add `--features overlay` (and `tray,overlay`) clippy runs to `justfile` `check`/`ci`/`fix` (`:25-42`) and the macOS CI job (`.github/workflows/ci.yml` ~44-51); on the Linux job add a **compile-only** `cargo clippy --features overlay` that exercises the no-op path (~82-89). Add `run-overlay: cargo run --features overlay`.

### Acceptance criteria
1. `cargo run --features overlay` opens a transparent, borderless, always-on-top window pinned to the active display's chosen anchor; it floats over other apps and across Spaces.
2. Opening it does **not** deactivate the foreground app; with TextEdit/VS Code foreground, opening the overlay leaves their text caret active. `[codex #6]`
3. A demo background task flips `OverlayState` after N seconds via the `WeakEntity<Overlay>` and the window repaints; verified no `save()`/IO on the render path.
4. Closing the main window closes the overlay, clears `OverlayHandle`, and the demo task exits; a push issued after close is a no-op (no panic, no `Err` unwrap). `[codex #8]`
5. `OverlayState` transition logic has ≥3 unit tests (pattern: `command_palette.rs:754`).
6. Scenario checks pass: second monitor (anchors to the active display), enter/leave a fullscreen Space, display hot-unplug while open (no crash), sleep/wake, and `--features tray,overlay` combined. `[codex #10]`
7. `just ci` green: default, `--features tray`, `--features overlay`, on macOS and Linux (Linux overlay = no-op). Default `cargo run` unchanged. `unsafe_code` stays zero in #1; no `todo!()`/`dbg!()`; every `update`/`open_window` `Result` handled.

### Testing
| Layer | What | Count |
|---|---|---|
| Unit | `OverlayState` transitions / anchor math | +3 |
| Manual | scenario checklist (criteria 1,2,4,6) | checklist |

### Effort
~2d `[codex: +0.5d for lifecycle + scenarios]`: 3h window factory + anchoring · 3h status-push (WeakEntity/Global) + lifecycle · 2h settings + feature gate + Linux no-op + CI matrix · 3h tests + scenario verify.

### Out of scope
Visible surface content beyond a placeholder (#2/#3); no-steal-on-click and click-through (#4).

---

## CHILD #2 — Top-right rail surface (generic job status + action buttons)

> **Redesign (2026-06-10).** This was the "status-icon row." It is now the **top-right
> rail** (`src/overlay/rail.rs`, a `Rail` view) — its own small transparent window, **no
> `Root` wrapper**. The generic job-status icons are kept as the top section; below a
> divider sit **3 square clickable action buttons** with a basic hover state. The
> "strictly generic, no agent/LLM-specific UI" rule still holds.

> **Design decision (user):** strictly generic — the status section represents *any* async
> job, with **no agent/LLM-specific UI or demo**. It's the visual proof of the background-job
> spine (`docs/background-jobs.md`). "Agent" framing is intentionally out.

### Context
A small frosted vertical panel, fixed top-right, sized to content (≈76×300). Three
stacked sections: ambient job-status icons (the spine proof — animate while running,
settle when done) → a thin divider → three square action buttons that demonstrate a
clickable hover control. The user asked for "larger icons or square buttons in a
vertical line that are clickable / have a basic hover state, that's it."

### Implementation details
- **Window:** opened by `mod.rs` as a transparent `PopUp` at `OverlayAnchor::TopRight`,
  with the view entity returned **directly** (`cx.new(|cx| Rail::new(..))`, no `Root`) so
  the window stays transparent. Only the rounded frosted panel paints.
- **Frosted panel:** `v_flex().rounded_xl().border_1().border_color(theme.border)
  .bg(theme.popover.opacity(0.85)).shadow_lg()`. No real blur/vibrancy (out of scope).
- **Section 1 — job-status icons:** one icon per `JobStatus` (the enum from
  `docs/background-jobs.md` §2, now in `src/overlay/status.rs`). `Running` pulses its
  opacity via `with_animation` + `pulsating_between` (`gpui/src/elements/animation.rs:52,247`);
  `Done`/`Failed` settle static in distinct theme colors (`success`/`danger`).
  `AnimationExt` is **not** in the prelude — `use gpui::{Animation, AnimationExt}`. The
  animated arm is an opaque element — never attach `.id()`/`.on_click()` after
  `with_animation`. Driven by the demo spine (the `WeakEntity` push from `mod.rs`).
- **Section 2 — divider:** a thin `div().h(px(1.0))` in `theme.border`.
- **Section 3 — 3 square action buttons:** styled `div`s, **not** gpui-component `Button`
  (full control over the square shape + hover; avoids `Root`/tooltip coupling).
  `div().id(..).size(px(34.0)).rounded_lg().bg(rest).hover(|s| s.bg(hover)).active(|s|
  s.bg(press)).on_click(..)`. The generic set:
  - **Pin** — a toggle (`IconName::Star` → `IconName::StarFill`) that flips a `pinned: bool`
    and stays **visibly filled** (primary tint) while active.
  - **Eye** and **Bell** — momentary buttons.
  - Each click prints a one-line stderr marker (e.g. `overlay rail: pin -> true`,
    `overlay rail: eye clicked`) so "clickable" is verifiable.
  No tooltips, no `Kbd` chips.
- **Theme** via `cx.theme()` (`.popover`, `.border`, `.primary`, `.success`, `.danger`,
  `.muted_foreground`); copy the colors out of the `&Theme` borrow before the button
  `cx.listener` closures re-borrow `cx` as `&mut`.

### Acceptance criteria
1. With `--features overlay`, the rail shows a small top-right frosted panel — **no
   full-window dark box** (transparent pixels show the app/desktop behind).
2. A mock task marks a job "running" → its icon pulses; "done"/"failed" → it settles in a
   distinct color, all from a background task (no UI-thread blocking).
3. The 3 square buttons each have a visible hover (bg tint) + press state and are
   clickable; each click has an observable effect (stderr marker and/or, for pin, a
   filled active state). Repaints only the rail entity (smallest-entity `cx.notify()`).
4. `just ci` green across the matrix.

### Testing
| Layer | What | Count |
|---|---|---|
| Unit | `JobStatus` status reducer (kept in `status.rs`) | +2 |
| Manual | icons pulse while running, settle on done; buttons hover + click markers | checklist |

### Out of scope
Any agent/LLM-specific UI (deliberately generic — wiring a real agent is downstream);
click-to-expand a job; persisted job history; tooltips / `Kbd` chips on the buttons.

---

## CHILD #3 — Bottom-center recording pill surface

> **Redesign (2026-06-10).** This was the "Wispr-style HUD pill," a
> dormant→toolbar→capsule→banner state machine with `HudState`, `Transition` morphs,
> tooltips, and `Kbd` chips. **All of that is removed** as over-built. It is now the
> **minimal recording pill** (`src/overlay/pill.rs`, a `RecordingPill` view) — its own
> small transparent window, **no `Root` wrapper**: a frosted pill with a single record
> button and a `recording: bool`. The user "liked the space-bar recording and the look of
> the individual elements," not the multi-state toolbar.

### Context
A small frosted compact pill, fixed bottom-center, sized to content (≈220×64): a circular
record button that pulses/colors red while "recording." The minimal version of the
space-bar record mock — real audio/dictation stays Phase 3.

### Implementation details
- **Window:** opened by `mod.rs` as a transparent `PopUp` at `OverlayAnchor::BottomCenter`,
  with the view entity returned **directly** (`cx.new(|cx| RecordingPill::new(..))`, no
  `Root`) so the window stays transparent. Only the rounded frosted pill paints.
- **Pill + record button:** `h_flex()` frosted container (`rounded_full`, translucent
  `theme.popover`, border, shadow) holding one circular record button — a styled `div`
  with `.id("pill-record")`, `.hover()`/`.active()` tints, `.on_click(..)`. The inner
  record dot is a child: at rest a static `theme.muted_foreground` circle; while
  `recording` a red (`theme.danger`) circle whose opacity **pulses** via `with_animation`
  + `pulsating_between`. (`with_animation` returns an opaque element — it is the dot, a
  child, never the clickable surface; the `.id()` button is the stateful parent that
  carries `.on_click`.)
- **Toggle — two paths (Decision: "robustly work and make sense, don't overengineer"):**
  1. **Click** the record button → flips `recording` + `cx.notify()` (pure
     `RecordingPill::toggled` reducer).
  2. A `ToggleRecording` action (`gpui::actions!`) bound to **`space`** (`cx.bind_keys` in
     `mod.rs`), handled via `cx.on_action`, flips the pill through its weak handle +
     `cx.notify()` — **while the Deck window is focused.** The binding is scoped to a
     non-input context (`!Input && !NumberInput && !SearchPanel`) so a space typed in a
     text field is never eaten. **No global hotkey** — over-other-apps space is Phase 3
     (overlay windows never hold focus, so it would need a global hotkey = real-app
     territory).
- **P2 focus spike (kept):** the record-button click also calls
  `harden::log_focus_state(window)`, logging the panel's `isKeyWindow` + frontmost app —
  the objective signal for the over-other-apps verdict (see §4 SPIKE RESULT).

### Acceptance criteria
1. With `--features overlay`, a small bottom-center frosted pill is visible — **no
   full-window dark box** (transparent pixels show the app/desktop behind).
2. Clicking the record button toggles a visible recording state (red dot + pulse);
   clicking again stops it. Pressing **`space` while the Deck window is focused** also
   toggles it; a space typed in a text input does not.
3. Repaints only the pill entity (smallest-entity `cx.notify()` rule).
4. **Gated on #4 (v1):** with another app foreground, clicking the record button does not
   steal its keyboard focus (logged via `log_focus_state`); transparent-area clicks pass
   through.
5. `just ci` green across the matrix.

### Testing
| Layer | What | Count |
|---|---|---|
| Unit | `RecordingPill::toggled` recording reducer | +1 |
| Manual | record click + `space`-while-focused toggle; red pulse; focus-spike log | checklist |

### Out of scope
Real microphone/dictation; sending text to other apps; a global hotkey so `space` records
over other apps (Phase 3); click-without-focus-steal (#4); tooltips / `Kbd` chips; the
removed toolbar/capsule/banner states and language popover.

---

## CHILD #4 — macOS interaction hardening (no-steal-on-click + click-through)

> **v1 GATE (user decision):** the HUD must work over OTHER apps from day one (the true Wispr gesture). **The P2 spike runs immediately after #1, before #3's full build** — its central mechanism is unproven on GPUI's single-NSView model and must be demonstrated first; if it fails, scope is renegotiated rather than discovered late. `[codex MUST_FIX_FIRST]`

### Three distinct OS-level problems (commonly conflated)

| # | Problem | What the user sees | v1 state |
|---|---------|--------------------|----------|
| P1 | **App activation** — a normal window click makes Deck frontmost, deactivating the app you were in | Your editor visibly loses active-app chrome | ✅ solved by `WindowKind::PopUp` non-activating NSPanel (`gpui_macos/src/window.rs:714-715`) — **as long as we never call `cx.activate(true)` to summon it** `[codex #6]` |
| P2 | **Key-window focus steal** — even non-activating, clicking the panel makes it *key*, so the other app's focused text field loses first-responder | You click the mic, your keystrokes stop reaching your editor | ❌ GPUI hardcodes `canBecomeKeyWindow → YES` (`gpui_macos/src/window.rs:364-367`); needs work |
| P3 | **Click absorption** — clicking a *transparent* pixel of the overlay window consumes the event instead of passing it to the app below | Clicks "die" on the invisible overlay rectangle | ❌ no passthrough API in gpui |

P2 and P3 are independent; fixing one does not fix the other.

### P2 — Key-window focus steal  (SPIKE FIRST)
**Candidate fix: per-instance `setBecomesKeyOnlyIfNeeded: YES`** on the panel (NOT class swizzling). It makes the panel become key only when a view in it reports `needsPanelToBecomeKey`. This is **not contradictory** with GPUI's hardcoded `canBecomeKeyWindow → YES` `[codex fact-correction]`: `YES` merely *permits* key status; `becomesKeyOnlyIfNeeded` gates the *click-to-key* behavior, so the two compose.

**Why it's a spike, not an assertion `[codex #2]`:** GPUI renders the whole window into *one* custom `NSView`, not native `NSControl`s, so it is unproven that a focused GPUI text input will report `needsPanelToBecomeKey`. The spike must:
1. Open a `PopUp` overlay, apply `setBecomesKeyOnlyIfNeeded(true)`.
2. With TextEdit foreground holding a caret, click a HUD **button** → assert TextEdit keeps focus (P2 fixed).
3. Focus a GPUI **text input** inside the HUD → determine whether it can receive typing at all. If not, **document "no in-HUD text input"** as a known limitation (the Wispr model is global-hotkey-driven, so this is acceptable) and proceed; if yes, great.

**Why NOT swizzle `canBecomeKeyWindow → NO`:** it's a class method on the shared `GPUIPanel` class (`gpui_macos/src/window.rs:125-129`), hitting every `PopUp`/`Floating`/`Dialog` window app-wide and brittle across `just bump-gpui`; it's also too strong (a never-key panel can never host text and never gets `keyDown:`). Per-instance is strictly safer. (Blast radius today is small — gpui-component opens only `WindowKind::Normal`, verified — but still.)

### P3 — Click absorption / passthrough
**`setIgnoresMouseEvents` is whole-window, all-or-nothing** — turn it on and the HUD's own buttons stop receiving clicks. So it is only a tool for fully passive overlays (cursor trails / screen-draw-idle, out of v1 scope), never the interactive HUD.

**v1 answer: size the window to content + a defined dead-zone budget.** `[codex #3,#4]`
- The fixed-canvas window must be sized to the **max** state footprint (no atomic origin/bounds setter; `resize()` exists but drifts when re-anchoring). Define a hard budget, e.g. **HUD ≤ 460×160 px**, banner text truncates/scrolls rather than spanning the screen.
- **Bottom-center is the worst place to absorb clicks** (dock, app toolbars). Mitigations, pick one in the spike: (a) cap each state's footprint and accept a small documented dead zone; (b) anchor the HUD a few px above the dock; (c) split dormant/expanded/banner into *separate* tightly-sized windows so the dead zone matches the visible pixels. Acceptance must include a "click the transparent area, the app below receives it" test for whatever option is chosen.

**Dynamic per-region passthrough is blocked in pure GPUI `[codex #7]`:** `on_mouse_event::<MouseMoveEvent>` exists (`window.rs:4284`), but once `setIgnoresMouseEvents(YES)` is set the window receives no events, so it can't detect the cursor re-entering an interactive region. This is a *GPUI-only* limitation — a native global event monitor (`NSEvent.addGlobalMonitorForEvents`) or a second always-passthrough window are viable later. Defer.

### objc2 bridge — one main-thread helper, no stored pointers `[codex #5]`
Provide a single function, called once on the main thread after open:
```
fn harden_panel(window: &Window) {           // macOS only
    // 1. window.window_handle() -> RawWindowHandle::AppKit { ns_view }   (window.rs:5933; gpui_macos:1794)
    // 2. ns_view.window  -> the NSWindow/NSPanel                          (guard nil: detached view)
    // 3. confirm it responds as NSPanel, then setBecomesKeyOnlyIfNeeded(true)
    // never store the native pointer; do everything inside this call
}
```
Threading: must run on the main thread (objc2 `MainThreadMarker`, as `tray.rs:99`). Nil-guard the detached-view case. Re-apply if the window is recreated (multi-monitor re-anchor).

### Unsafe / dependency budget  `[codex fact-corrections]`
- "No new crate" is accurate; but enabling `--features overlay` on macOS **does pull the existing** `objc2`/`objc2-app-kit`/`objc2-foundation` (already declared, macOS-only) — so the feature's macOS dep set grows even though no new crate name appears.
- **Likely one scoped `unsafe`:** recovering the `NSWindow` from the raw `NSView` and downcasting to `NSPanel` probably needs `msg_send!`/pointer work even if `setBecomesKeyOnlyIfNeeded` itself is a safe objc2-app-kit wrapper. Budget for a single `#[allow(unsafe_code)] // SAFETY:` block (the manifest is `deny`-not-`forbid` for exactly this, `Cargo.toml:23`). The spike resolves whether any unsafe is truly required.

### Acceptance criteria (#4)
1. **Spike gate:** a written result proving (or disproving) that `setBecomesKeyOnlyIfNeeded(true)` keeps TextEdit's caret active when a HUD button is clicked, plus the in-HUD-text-input verdict. The rest of #4 proceeds only if the spike confirms P2 is achievable.
2. The overlay is sized to content within the dead-zone budget; a click on a transparent overlay pixel reaches the app below for the chosen P3 option.
3. Any `unsafe` carries `// SAFETY:` + scoped `#[allow(unsafe_code)]`, on the main thread, storing no native pointer; `clippy -D warnings` stays green.
4. `just ci` green across the matrix.

### Effort
spike 0.5d + ~1.5d: 0.5d spike (P2 focus test + objc2-safe-wrapper check) · 0.5d `harden_panel` + main-thread guard · 0.5d footprint sizing + dead-zone/transparent-click audit.

### Out of scope (#4)
Full-screen click-through layers; dynamic per-region passthrough; global cursor tracking; global hotkeys — all Phase 3.

---

## Decision log (all resolved)
1. **#3 granularity:** keep #3 as one issue; split only if it balloons during implementation.
2. **#4 as v1 gate:** YES — the HUD must work over other apps; the P2 spike runs right after #1 to de-risk the riskiest unknown early.
3. **#2 framing:** strictly generic async-job status, **no agent/LLM-specific UI**.
4. **Doc:** promoted to `docs/overlay.md`, committed.
5. **Platform:** macOS-only v1; Linux `--features overlay` compiles to a no-op (LayerShell deferred).

## Codex review — verdict & disposition
- Score: 5/10 (v1 draft) → revised above.
- Applied: #1 status-push root-type bug, #3 lifecycle, #4 P2-as-spike, P3 resize wording + dead-zone budget + bottom-center risk, objc2 helper, unsafe/dep wording, Linux/CI no-op, scenario acceptance criteria, "don't `cx.activate` on summon".

---

# Implementation handoff (for a fresh agent thread)

> **You are picking this up cold. Read §0–§2, then execute the §3 sequence in order. T0 (the spike)
> is mandatory and first — it decides whether the v1 scope is even feasible. Do not build #1's full
> surface before T0 returns PASS.** Everything you need is below or cited; you should not need to
> re-derive the design.

## §0 — Orientation

**What you're building.** A reusable "floating overlay surface" for Deck (a GPUI + gpui-component
macOS/Linux desktop-app *starter*): a transparent, borderless, always-on-top window you mount
ambient status/controls into, gated behind `--features overlay`. It exists so every fork gets a
first-class way to show async/background work (the thing Wispr Flow, Raycast, Linear lean on). The
visual reference is the Wispr Flow dictation HUD: a bottom-center pill that idles as a tiny handle,
expands on hover into a toolbar, morphs into a recording capsule, and drops a guidance banner.

**Why these specific decisions** (see "Decision log" above): macOS-only v1 (Linux is a compiling
no-op); #4 (over-other-apps hardening) is a v1 gate, so its riskiest unknown (P2) is spiked first;
#2 is strictly generic job status with no agent-specific UI; the morph is done by animating a child
inside a fixed-size window (GPUI has no atomic window move/bounds setter).

**Prime constraints (from `CLAUDE.md` — non-negotiable):**
- Definition of Done = `just ci` green (fmt + clippy `-D warnings` on **both** default and
  `--features tray`, plus `cargo test`). Add `--features overlay` to that matrix. Paste evidence.
- **No new deps** without explicit approval. The overlay's objc2 calls reuse the **existing**
  `objc2`/`objc2-app-kit`/`objc2-foundation` (already declared macOS-only under `--features tray`,
  `Cargo.toml:65-68`) — surface them under `overlay` too via `dep:` entries; that's allowed (no new
  crate). Anything else (HTTP client, gpui_tokio, etc.) is out of scope here.
- `unsafe_code` is **deny** (not forbid): a genuinely-needed block gets `// SAFETY:` + a scoped
  `#[allow(unsafe_code)]`. Aim for zero; budget at most one in T0/T6 (the raw-handle bridge).
- Never block the render thread on I/O; `cx.notify()` the smallest entity; persist via
  `Settings::save_best_effort()` off the hot path. (`docs/LEARNINGS.md` §17.)
- No `todo!()`/`dbg!()`; never drop a `Result` (`unused_must_use` denied).

## §1 — Verified API quick-reference (don't re-research; all checked against the pinned source)

Pinned source (this machine): GPUI `~/.cargo/git/checkouts/zed-a70e2ad075855582/86effff/crates/{gpui,gpui_macos}`,
gpui-component `~/.cargo/git/checkouts/gpui-component-95ce574d8a0da8b8/dadfca9`. (GPUI rev `86effffd…`,
component rev `dadfca9…` from `Cargo.lock`; on another machine locate via `cargo metadata`.)
Copyable examples live in `…/gpui/examples/`: `window_positioning.rs`, `animation.rs`, `opacity.rs`,
`painting.rs`, `popover.rs`, `move_entity_between_windows.rs`.

| Need | Symbol / call | Location |
|---|---|---|
| Open a window | `cx.open_window(opts, \|window, cx\| …) -> WindowHandle<Root>` | `gpui app.rs:1136`; pattern `main.rs:134-138` |
| Always-on-top + non-activating + all-Spaces + hover-while-inactive | `WindowKind::PopUp` (NSPanel, `NSPopUpWindowLevel`=101, `CanJoinAllSpaces\|FullScreenAuxiliary`, `NSTrackingActiveAlways`) | `gpui_macos/src/window.rs:714-717,904-927,919` |
| Borderless | `titlebar: None` | `gpui_macos/src/window.rs:689-708` |
| Transparent bg | `WindowBackgroundAppearance::Transparent` | `gpui/src/platform.rs:1693` |
| Root wrap — **only** when you need tooltip/notification/modal layers (it paints an opaque `theme.background`, so the redesigned transparent surfaces skip it) | `Root::new(view, window, cx)` | gpui-component `root.rs:89,513-531`; `main.rs:135-137` |
| Active display + bounds (anchoring) | `cx.primary_display()`/`cx.displays()`; `PlatformDisplay::visible_bounds()` | `app.rs:1192,1197`; `platform.rs:262` |
| Window move/bounds | **none** (`resize()` exists `window.rs:2217` but drifts — use fixed canvas) | `platform.rs:614-660` |
| Run off UI thread (Send) | `cx.background_executor().spawn(fut)` | `executor.rs:89` |
| Run async touching UI | `cx.spawn(async move \|cx\| …)` → `AsyncApp` | `async_context.rs:204` |
| Timer / cancel | `…background_executor().timer(dur)`; **drop `Task` = cancel** | `executor.rs:162` |
| Raw NSWindow (for objc2) | `window.window_handle()` → `RawWindowHandle::AppKit{ ns_view }` → `[ns_view window]` | `window.rs:5933`; `gpui_macos/src/window.rs:1794-1798` |
| objc2 on main thread | `MainThreadMarker::new()` + `objc2_app_kit` (model: dock policy) | `tray.rs:94-104` |
| `canBecomeKeyWindow` hardcoded YES (the P2 problem) | `build_window_class` | `gpui_macos/src/window.rs:364-367` |
| Repeating animation / pulse | `with_animation` + `pulsating_between` (`AnimationExt` **not** in prelude) | `gpui/src/elements/animation.rs:52,247` |
| Expand/collapse morph | gpui-component `Transition` (`.width/.height/.fade/.slide_y`) | gpui-component `animation.rs` |
| Hover-to-expand | `.on_hover(&bool)` (Stateful — needs `.id()`) + `cx.notify()` | `gpui` div |
| Tooltip + shortcut chip | `Tooltip::new(..).action(&A, ctx).build(..)`; `Kbd`/`Kbd::binding_for_action` | gpui-component `tooltip.rs`, `kbd.rs` |
| Spinner | `Spinner` | gpui-component `spinner.rs` |
| Settings (add fields) | `Settings` struct + `Default` (`#[serde(default)]`); `save_best_effort()` | `settings.rs:43-61,108` |
| Feature flag + wiring | `[features] overlay = []`; `#[cfg(feature="overlay")] mod overlay;` + `overlay::install(cx,&settings)` | `Cargo.toml:60`; `main.rs:15-16,142-143` |
| Pure-logic unit-test pattern | `fuzzy()` + `#[cfg(test)] mod tests` | `command_palette.rs:217,754` |

## §2 — Build / verify loop
- Type-check fast: `cargo check --features overlay`. Lint like CI: `cargo clippy --features overlay --all-targets -- -D warnings` (use `--message-format=short` for self-correction).
- Full gate before declaring any task done: **`just ci`** — and add `--features overlay` (+ `tray,overlay`) to `justfile` `check`/`ci`/`fix` (`:25-42`) and the CI jobs (`.github/workflows/ci.yml` macOS ~44-51, Linux ~82-89). Run the app: `cargo run --features overlay` (add a `run-overlay` recipe).
- `just fix` auto-applies clippy + fmt. Loop `just fix` → `just ci`.

## §3 — The sequence

### T0 — P2 SPIKE (do first; gates everything) 🔴
**Question to answer:** does a GPUI `WindowKind::PopUp` window with `setBecomesKeyOnlyIfNeeded(true)`
on its NSPanel **keep keyboard focus in another app** when you click a button in the overlay? This is
unproven because GPUI renders into a *single* custom NSView (so AppKit's "does any subview need to
become key" logic is untested for GPUI widgets) and GPUI hardcodes `canBecomeKeyWindow=YES`
(`gpui_macos/src/window.rs:364-367`). If it fails, the over-other-apps v1 gate is infeasible as
specced — escalate before building more.

**Build the minimum (this is also the seed of #1):**
1. Add `overlay = []` to `[features]`; create `src/overlay/mod.rs` with `#[cfg(feature="overlay")]`
   wiring in `main.rs` (`mod` + `install(cx)` after the main `open_window`). Gate the window body
   `#[cfg(target_os="macos")]`; non-macOS `install` = no-op.
2. Open a `WindowKind::PopUp`, transparent, borderless window ~320×120, anchored bottom-center of the
   active display, wrapped in `Root::new`. Render one gpui-component `Button` ("Click me") that
   increments a counter on a small `Entity` + a label showing the count (proves the panel still gets
   mouse clicks).
3. Write `fn harden_panel(window: &Window)` (macOS): `window.window_handle()` →
   `RawWindowHandle::AppKit{ ns_view }` → get `[ns_view window]` → call
   `setBecomesKeyOnlyIfNeeded(true)`. Run on the main thread (`MainThreadMarker`, like `tray.rs:99`).
   Nil-guard the detached-view case. Never store the native pointer. If objc2-app-kit lacks a safe
   wrapper, use a single `// SAFETY:` + scoped `#[allow(unsafe_code)]` block.
4. **Instrument an objective signal** (so the verdict isn't only eyeballed): log, right after a button
   click, the panel's `isKeyWindow` and `NSApp.isActive` (or
   `NSWorkspace.frontmostApplication.localizedName`). Expectation on PASS: panel `isKeyWindow == false`
   and the frontmost app stays the other app.

**Test protocol (human-in-the-loop, ~5 min):**
1. Open TextEdit, click into a document so it has a blinking caret; type a few chars.
2. `cargo run --features overlay`. Overlay appears bottom-center; TextEdit stays frontmost. Type — text must still land in TextEdit (the window opened `focus:false`, non-activating).
3. **Click the overlay's "Click me" button.** Then type again.
   - **PASS:** TextEdit keeps its caret and your keystrokes still land there; logs show panel `isKeyWindow == false`; the counter still incremented (button got the click).
   - **FAIL:** TextEdit loses the caret / keystrokes stop landing / logs show the panel became key.

**If FAIL — escalation ladder (try in order, document each):**
1. Confirm `harden_panel` actually ran on the real panel (log the class name; expect `GPUIPanel`/NSPanel).
2. Instance-swizzle just this panel's `canBecomeKeyWindow → NO` (objc2) and re-test; verify buttons
   still click (a never-key panel that still gets mouse events = acceptable, at the cost of no in-HUD
   text input — document that limitation).
3. If neither holds, the over-other-apps gesture can't be done purely in-process → **stop and report**:
   the v1 #4 gate needs renegotiation (drive via a global hotkey in Phase 3, or make the HUD
   display-only when over other apps). Do not silently proceed.

**T0 acceptance:** a written **SPIKE RESULT** appended to §4 below (PASS/FAIL + the observed behavior +
which mechanism worked); button-click registers; `just ci` green (objc2 clean or scoped-unsafe).

### T1 — #1 primitive proper (after T0 PASS)
Flesh the skeleton into the full primitive per **CHILD #1**: `OverlayState`/`OverlayAnchor` enums in
`src/overlay/state.rs` (+ ≥3 unit tests), the window factory + bottom-center/anchor math, `OverlayHandle`
Global holding `WeakEntity<Overlay>` + the `WindowHandle`, settings fields (`overlay_enabled`,
`overlay_anchor`) with `save_best_effort`. **Acceptance:** CHILD #1 criteria 1,2,5,7.

### T2 — #1 status-push spine + lifecycle
The `WeakEntity<Overlay>` push from a background task (mock task flips state on a timer); close overlay
on main-window close; clear the Global on overlay close; pushes after close no-op (`upgrade()→None`);
don't `cx.activate(true)` on summon. **Acceptance:** CHILD #1 criteria 3,4 + lifecycle.

### T3 — #1 CI matrix + scenario tests
Add `--features overlay` to justfile + both CI jobs (Linux = compile-only no-op). Run the scenario
checklist (multi-monitor, fullscreen Space, hot-unplug, sleep/wake, `tray,overlay`). **Acceptance:**
CHILD #1 criteria 6,7 green.

> **`[redesign 2026-06-10]`** T4/T5 below are superseded by the rewritten CHILD #2 / #3.
> T4 is now the top-right **rail** (status icons + divider + 3 square action buttons); T5
> is the bottom-center **recording pill** (record button + `space`-while-focused toggle +
> red pulse), **not** the HUD state machine. Both render as separate `Root`-less
> transparent windows. The verbatim steps below are kept as the original plan's history.

### T4 — #2 status-icon row (strictly generic)
Top-right vertical row of icons, each a generic `JobStatus` (from `docs/background-jobs.md` §2),
animating while running via `with_animation`/`pulsating_between`, settling when done; driven by mock
generic jobs through the T2 spine. **No agent/LLM-specific UI.** **Acceptance:** CHILD #2.

### T5 — #3 Wispr HUD pill
`HudState { Dormant, Expanded, Active{label,amplitudes}, Banner{text} }` + split render methods; morph
via `Transition`; hover-expand; toolbar Buttons + `Tooltip`+`Kbd` chips; waveform bars + pulse dot;
banner. Stay within the dead-zone budget (≤460×160). **Acceptance:** CHILD #3 (incl. criterion 6,
gated on T6).

### T6 — #4 P3 hardening + finalize
Finalize `harden_panel` (P2 from T0) and the P3 footprint/transparent-click handling (size-to-content
within budget; verify a click on a transparent overlay pixel reaches the app below for the chosen
option). **Acceptance:** CHILD #4 criteria 1-4.

## §4 — SPIKE RESULT

**Status (2026-06-09): SCAFFOLD READY — focus verdict pending one ~5-min human run.** Everything
machine-verifiable is green; the one thing that needs a real macOS GUI session (does clicking a HUD
button over TextEdit keep TextEdit's caret) has not been observed in this headless build environment,
so it is **not** marked PASS. No result was fabricated.

### What is built and compiles green (verified)
- **P2 mechanism wired:** `src/overlay/harden.rs::harden_panel()` recovers the `NSPanel` from gpui's
  raw `NSView` (`HasWindowHandle` → `RawWindowHandle::AppKit` → `NSView::window()` →
  `Retained::downcast::<NSPanel>()`) and calls `panel.setBecomesKeyOnlyIfNeeded(true)` on the main
  thread (`MainThreadMarker`), storing no native pointer. Called inside the `open_window` build closure.
- **objc2 fact-corrections vs. the spec draft:** the call chain uses **safe** objc2-app-kit 0.3.2
  wrappers (`window()`, `downcast`, `setBecomesKeyOnlyIfNeeded`) — **no `msg_send!`**. Exactly **one**
  scoped `unsafe` per fn (`Retained::retain(ns_view_ptr)`), each with `// SAFETY:` + `#[allow(unsafe_code)]`.
  `clippy -D warnings` stays green across the matrix; `unsafe_code` count = 2 (both in harden.rs, both
  the raw-handle bridge — none elsewhere).
- **Objective instrumentation:** `log_focus_state()` logs the panel's `isKeyWindow` and
  `NSWorkspace.frontmostApplication().localizedName()` after a click, wired into the HUD toolbar's
  record button `on_click` — so the verdict is logged, not eyeballed.
- **No new crate / no lock churn:** the only Cargo.lock delta is a single `raw-window-handle` edge on
  the `deck` package (rwh 0.6.2 was already in the graph via gpui); macOS-only, optional, under `overlay`.

### Run protocol to fill the verdict (do this on macOS)
1. Open TextEdit, click into a document so it has a blinking caret; type a few chars.
2. `cargo run --features overlay` (the overlay opens bottom-center; default anchor = the HUD pill).
   TextEdit must stay frontmost and keep accepting your keystrokes (window opened `focus:false`,
   non-activating `PopUp`).
3. Hover the dormant handle → the toolbar expands. **Click the record button.** Then type again.
   - **PASS:** TextEdit keeps its caret and keystrokes still land there; logs show
     `panel isKeyWindow = false` and `frontmost app = "TextEdit"`; the click still registered.
   - **FAIL:** TextEdit loses the caret / keystrokes stop / logs show the panel became key →
     follow the §3 T0 escalation ladder (confirm `harden_panel` ran on the real `GPUIPanel`; then
     try a per-instance `canBecomeKeyWindow → NO` swizzle; if neither holds, renegotiate the #4 gate).

### Recommendation
The mechanism is the spec's recommended one and is now real, safe, and compiling. Proceed to the human
focus run to convert this to PASS/FAIL; treat the over-other-apps acceptance (CHILD #3 criterion 6,
CHILD #4 criteria 1–2) as **provisional** until that run is recorded here.

---

# §5 — Configuration & Settings UI

**Decision (2026-06-09, with the maintainer).** The v1 wiring shipped a confusing **double-gate**:
`--features overlay` (build) **and** an `overlay_enabled` setting that defaulted **off** with **no UI**,
so compiling the feature showed nothing — a dead runtime gate that only annoyed whoever works on the
template, with no benefit to forkers. It is also inconsistent with `--features tray`, which gates on the
build flag alone. This section locks the replacement. **The gating fix (default-on + env override) is
implemented; the in-app Settings UI with live apply is the remaining deferred work.**

> **Redesign update (2026-06-10).** There is no longer one overlay window with a
> user-chosen anchor — there are **two fixed surfaces** (top-right rail + bottom-center
> pill). So the **anchor is no longer a user preference**: the `overlay_anchor` setting,
> its `DECK_OVERLAY_ANCHOR` env override, and the planned **anchor picker** are all
> **removed**. `OverlayAnchor` survives only as an **internal positioning helper** in
> `src/overlay/state.rs` (rail = `TopRight`, pill = `BottomCenter`). `overlay_enabled` +
> `DECK_OVERLAY=0/1` are retained as the master on/off for both surfaces. The deferred
> work below is reframed from "anchor picker" to **per-surface show/hide** (toggle the
> rail and the pill independently), still deferred.

### Locked model
- **The Cargo feature is the ONLY compile-time gate.** `--features overlay` decides whether the overlay
  code + its macOS-only `objc2` / `raw-window-handle` deps compile — exactly like `--features tray`.
  The feature flag never doubles as a *runtime* enable gate.
- **Runtime preferences are real, persisted, and applied live** (the "complete, forkable feature"
  pattern, not a dead gate):
  - `overlay_enabled: bool` — **defaults `true`** (so `cargo run --features overlay` shows the overlay
    immediately, mirroring how `--features tray` just works). It is the single persisted "show overlay"
    preference (covers both surfaces), flipped by a **Settings UI toggle**. Toggling it
    **opens/closes the live overlay windows** — no restart.
  - ~~`overlay_anchor: OverlayAnchor`~~ — **removed (redesign).** The two surfaces have
    fixed anchors; there is no user anchor choice. `OverlayAnchor` is now an internal
    `state.rs` helper only.
- **Env override for quick dev/test runs (and a fork hook), no JSON editing** — read in `overlay::install`:
  - `DECK_OVERLAY=1|0|true|false` — force the overlay on/off for this run.
  - ~~`DECK_OVERLAY_ANCHOR=…`~~ — **removed (redesign):** no anchor to pick.
  Unset → use the persisted setting. Unrecognized → warn on stderr + fall back. The parser is pure + unit-tested.

### Implemented (the gating fix)
1. **`settings.rs`** — `overlay_enabled` defaults `true`, so `cargo run --features overlay` shows the
   overlay immediately; no JSON editing. The Cargo feature is the only *build* gate.
2. **`overlay/mod.rs`** — `install` computes the effective `enabled` flag from the setting, overridable by
   the `DECK_OVERLAY` env var above (pure `parse_*` helper + tests).
3. **`main.rs`** — the `#[cfg(feature = "overlay")] overlay::install(...)` call honors the
   (default-on) `overlay_enabled` for the initial open.

### Deferred (the in-app Settings UI + live apply)
4. **`overlay/mod.rs`** — add runtime controls reusing the existing window factory + `harden_panel`:
   `show(cx)` / `hide(cx)` (or one `apply_settings(cx, &Settings)`). `hide` closes the overlay windows +
   clears the handle(s); `show` re-runs the factory + re-stores the global(s). **Re-apply `harden_panel`
   on every (re)open** (also the multi-monitor re-anchor case). The existing `on_window_closed` teardown
   must still hold; the overlay still never calls `cx.quit()` / `cx.activate()`. Optional follow-up:
   **per-surface on/off** — show/hide the rail and the pill independently (e.g. `show_rail` / `show_pill`
   bools) now that they are separate windows.
5. **`settings_view.rs`** — add an **Overlay** section, compiled only under `#[cfg(feature = "overlay")]`,
   with a "Show overlay" toggle (and, as the per-surface follow-up, separate rail/pill toggles — **no
   anchor picker**). On change: persist via `save_best_effort()` (off the hot path) **and** call the live
   `show`/`hide`. Wire through `Shell` like the existing theme/accent controls (mirror how `set_accent`
   already calls into a global / `tray::set_accent`). The env override above stays as the headless/dev path.
6. **Tests/CI** — matrix unchanged; add a pure unit test for any new reducer-style state if introduced.

### Why this is the right template pattern
Forkers get exactly one obvious switch — the Cargo feature — to include/exclude the capability **and its
deps**, plus a complete, copyable example of a persisted, Settings-driven, **live-applied** runtime
preference: the precise shape they will reuse for their own toggleable surfaces. No dead gates, and
consistent with `--features tray`.
