# Spec: Floating overlay surfaces for Deck

Status: DESIGN (reviewed) · Scope: macOS-only v1 · Audience: reusable Deck template flow
Branch: `hellno/hover-overlay-window` · Research backing: `.context/floating-overlay-design.md` (working notes, uncommitted)
Decision log: #4 (over-other-apps hardening) is a **v1 gate**; its P2 spike runs first.
Independent review: codex (gpt) scored the v1 draft **5/10** executability; this revision folds in its
ten findings (status-push root-type bug, P2 spike, resize wording, lifecycle, Linux/CI, dead-zone, objc2
helper, unsafe/dep wording). Corrections are marked `[codex]` inline.

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
| 2 | **Agent-icon row** surface (ambient async-agent status) | High | ~1d | #1 | none / none |
| 3 | **Wispr-style HUD pill** surface (dormant→toolbar→recording→banner) | High | ~2d | #1 | none / none |
| 4 | **macOS interaction hardening** (no-steal-on-click + click-through) — **v1 GATE** for #3; the HUD must work over other apps | Critical | spike + ~1.5d | #1 | none / likely 1 scoped unsafe |

### Dependency graph
```
#1 Overlay primitive ──> #4 SPIKE (P2: prove no-steal) ─┬─> #2 Agent-icon row
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
- **Settings:** add `#[serde(default)]` `overlay_enabled: bool` + `overlay_anchor: OverlayAnchor` to `Settings` (`settings.rs:43-61`; update `Default` `:52-61`). Persist via `save_best_effort()` (`settings.rs:108`) at a commit boundary only.
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

## CHILD #2 — Agent-icon row surface

### Context
The first real surface and the cheapest proof of the spine: a vertical row of icons, each an async agent, that animates ("jiggles"/pulses) while working and settles when done. Zero focus/click caveats — ambient status you read, not click.

### Implementation details
- Render inside the #1 overlay window, anchored top-right (`OverlayAnchor::TopRight`). Each agent = an icon element; "working" = a repeating animation via `with_animation` + `pulsating_between` (`gpui/src/elements/animation.rs:52,247`); jiggle = oscillating translate/rotate (ref `gpui/examples/animation.rs`). `AnimationExt` is **not** in the prelude — `use gpui::{Animation, AnimationExt}`.
- Agent list is a `Vec<AgentStatus>` on the `Overlay` entity; a background task mutates it + `cx.notify()` via the #1 spine (the `WeakEntity<Overlay>` path).
- Theme via `cx.theme()` (`.primary`, `.muted_foreground`) + gpui-component `v_flex`/`Icon` (mirror `welcome.rs:20-27`).

### Acceptance criteria
1. With `--features overlay`, the overlay shows a top-right vertical icon row.
2. A mock task marks an agent "working" → its icon animates; "done" → it settles, all from a background task (no UI-thread blocking).
3. Adding/removing agents repaints only the overlay entity (smallest-entity `cx.notify()` rule).
4. `just ci` green across the matrix.

### Testing
| Layer | What | Count |
|---|---|---|
| Unit | agent add/remove/status reducer | +2 |
| Manual | icons animate while working, settle on done | checklist |

### Effort
~1d: 3h row + per-agent animation · 2h board state + mock task · 1h tests + verify.

### Out of scope
Real agent integration; click-to-expand an agent; persisted agent history.

---

## CHILD #3 — Wispr-style HUD pill surface

### Context
The headline surface from the reference video: a bottom-center pill that idles as a tiny handle, expands on hover into a toolbar, morphs into a recording capsule, drops a guidance banner. Pure visual composition — real audio is Phase 3; no-steal/click-through is #4.

### Implementation details
- State machine `enum HudState { Dormant, Expanded, Active{label, amplitudes:Vec<f32>}, Banner{text} }` on the `Overlay` entity; `render_dormant/_toolbar/_capsule/_banner` split methods (mirror `welcome.rs` split-render style).
- Morph (size + opacity) via gpui-component `Transition` (`.width/.height/.fade/.slide_y`) inside the fixed canvas — never resize the window. A `div` can't transform-scale; animate real `.h()`.
- Hover-expand: `.on_hover(&bool)` (Stateful — needs `.id()`) + `cx.notify()`; the `PopUp` tracking area (`gpui_macos/src/window.rs:904-917`) fires hover even when the app is inactive.
- Toolbar buttons (`EN`/mic/polish/notes) = gpui-component `Button`; tooltips with shortcut chips via `Tooltip::new(..).action(&A, ctx).build(..)` (`tooltip.rs`) + `Kbd`/`Kbd::binding_for_action` (`kbd.rs`).
- Recording waveform = N `div` bars animated by `with_animation`; pulse "ready" dot via `pulsating_between`; `Spinner` (`spinner.rs`) for indeterminate.

### Acceptance criteria
1. With `--features overlay`, a bottom-center pill cycles Dormant→Expanded→Active→Banner via a mock trigger, with animated transitions.
2. Hovering the dormant handle expands the toolbar; leaving collapses it.
3. Each toolbar button shows a tooltip with its keyboard-shortcut chip.
4. Active state shows an animated waveform + cancel/confirm; Banner shows a full-width guidance row.
5. The HUD's max footprint stays within the documented dead-zone budget (#4/P3) so transparent areas don't blanket the bottom of the screen. `[codex #4]`
6. **Gated on #4 (v1):** with another app foreground, clicking a toolbar button does not steal its keyboard focus, and transparent-area clicks pass through.
7. `just ci` green across the matrix.

### Testing
| Layer | What | Count |
|---|---|---|
| Unit | `HudState` transition reducer | +4 |
| Manual | hover-expand, tooltips, waveform, banner | checklist |

### Effort
~2d: 4h state machine + transitions · 4h toolbar + tooltips + Kbd · 4h waveform + capsule + banner · 4h tests + polish.

### Out of scope
Real microphone/dictation; sending text to other apps; click-without-focus-steal (#4); the language popover's actual switching.

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

## Open questions for review
1. Effort/granularity on #3 (biggest) — split toolbar vs capsule/banner?
2. ~~Should #4 be a v1 gate?~~ **RESOLVED: yes (user decision).** The HUD must work over other apps; the P2 spike runs right after #1 to de-risk the riskiest unknown early.
3. Agent-row "jiggle": generic async-task status, or shaped specifically around Claude/coding-agent status?
4. Doc location: keep in `.context/`, or promote to `docs/overlay.md` and commit?

## Codex review — verdict & disposition
- Score: 5/10 (v1 draft) → revised above.
- Applied: #1 status-push root-type bug, #3 lifecycle, #4 P2-as-spike, P3 resize wording + dead-zone budget + bottom-center risk, objc2 helper, unsafe/dep wording, Linux/CI no-op, scenario acceptance criteria, "don't `cx.activate` on summon".
- Not yet decided (needs your call): open questions 2 (gate #3 on #4?) and 1/3/4.
