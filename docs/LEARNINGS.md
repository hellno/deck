# LEARNINGS — building a native desktop app on GPUI

Notes from building Deck on Zed's GPUI and gpui-component — the "why" behind the repo's
decisions, and direct answers to:
*how nice can the theme get? what icons can I use? how do I save preferences? can I have a
menu-bar icon, a dock icon, a tray-first app? how do app icons work? how much should I bundle?*

All API claims below were verified against the actual crate sources that Deck compiles
against: `gpui 0.2.2` and `gpui-component 0.5.1` (the published crates.io versions), plus the Zed
source tree for context. File/line refs point into those.

---

## 1. The big picture: three layers

```
┌─────────────────────────────────────────────┐
│ your app  (src/*.rs)                          │  ~700 lines, a few files
├─────────────────────────────────────────────┤
│ gpui-component 0.5   buttons, inputs, theme, │  shadcn-style kit
│                      TitleBar, Root, icons   │
├─────────────────────────────────────────────┤
│ gpui 0.2   windows, elements, flexbox,       │  Zed's UI framework
│            Metal renderer, actions, menus    │  (Zed's own crate)
└─────────────────────────────────────────────┘
```

GPUI is Zed's retained-mode, GPU-accelerated UI framework: a React-ish model (`Render` views that
hold state and re-render on `cx.notify()`), a flexbox API that reads like Tailwind
(`div().flex().gap_2().p_4()`), a Metal renderer on macOS, an async executor, and native plumbing
(windows, menu bar, key dispatch). gpui-component adds the *look*: a themeable component library.

The hard parts (native window, GPU text, event loop, retained UI) are **inherited**. What Deck
**adds** is the chrome that makes it feel like an app: a window that looks native, a menu bar,
shortcuts, a theme that isn't harsh, a place to store preferences, an icon, and a `.app`.

---

## 2. The dependency decision {#dependencies}

The confusion isn't "Zed gpui vs. a longbridge fork" — that's a myth. crates.io `gpui` **is Zed's own
official crate** (owners: Max Brunsfeld, Mikayla Maki, the `zed-industries` team; repo
`zed-industries/zed`; homepage `gpui.rs`). The real distinction is **two release channels of the same
Zed gpui**, plus a third-party mirror:

| Channel | What it actually is | Renderer | Use it when |
|---|---|---|---|
| **crates.io pair** — `gpui` 0.2.x + `gpui-component` 0.5.x | Zed's gpui *published as a periodic snapshot* (0.2.0–0.2.2 all shipped Oct 2025; none since) + Longbridge's kit pinned to it | **Metal (macOS) + Blade/Vulkan (Linux)** | **Fallback** — plain-stable, zero git deps, but freezes gpui at the Oct-2025 snapshot |
| **git pair** — `gpui` + `gpui_platform` + `gpui-component` (`main`) | Zed HEAD + Longbridge HEAD, how gpui-component is actually developed | **Metal (macOS) + wgpu (Linux)** since [PR #46758](https://github.com/zed-industries/zed/pull/46758) (merged 2026-02-13) | **This Deck's default** — fresh gpui; reproducible via committed `Cargo.lock`; bump ~monthly (pre-1.0 churn) |
| `gpui-unofficial` (Nate Butler) | an automated *tag-for-tag mirror* of Zed releases on crates.io (now 1.x) — **not** a "wgpu fork"; it renders with whatever the mirrored tag uses | inherits upstream | a crates.io path closer to HEAD — but **not** what gpui-component pins, so never mix it with gpui-component |

Two non-obvious facts behind this table:

- **Zed publishes `gpui` to crates.io only occasionally.** The whole 0.2.x line shipped in October 2025
  (`0.2.0` Oct 9 → `0.2.2` Oct 22), then it went quiet. So the *latest stable* `gpui` is still that
  Oct-2025 snapshot; Zed's `main` calls itself 0.2.2 but is many months ahead, and has since split gpui
  into `gpui` + `gpui_platform` + `gpui_web` + `gpui_macros` (none of the split crates are on crates.io
  yet). Tracking latest-stable gpui therefore advances only when Zed cuts a new release.
- **gpui-component lives on the git channel and snapshots to crates.io.** Its `main` depends on Zed
  *git* gpui (`gpui = { git = ".../zed" }`), and its README tells you to as well — but each *published*
  release pins the registry (`gpui-component 0.5.1` → `gpui = "^0.2.2"`), which is what lets the
  crates.io pair `cargo run` cleanly. It ships roughly monthly, so latest-stable gpui-component *does*
  move regularly.

The trap to avoid: **a published `gpui-component` is built against one specific `gpui`.** Mix in a
*different* gpui (e.g. `gpui-unofficial`, or a mismatched git rev) and both halves still compile, but
your `Render` view satisfies one crate's `Render` trait while `open_window` / `Root::new` want the
other's — `E0277` trait-mismatch errors at *your* call sites (the dependency crates compile fine, which
is what makes it baffling). Keep the pair matched.

**Decision: the matched git pair, bumped on a cadence** — `gpui` + `gpui_platform` from
`zed-industries/zed` and `gpui-component` (+ `-assets`) from `longbridge/gpui-component`, all via git.
This is the *only* way to pair fresh gpui with the component kit, and it's how gpui-component is itself
developed (and how Longbridge ships its own app). Reproducibility comes from the committed `Cargo.lock`
(exact commit pins); you bump ~monthly with `just bump-gpui`. Two costs, both small: the git channel
needs a **recent Rust stable** (pinned in `rust-toolchain.toml` to match Zed — gpui HEAD uses
just-stabilized `std` APIs like `cold_path`), and you absorb occasional pre-1.0 API drift on bumps (the
Oct-2025 → mid-2026 jump took *four* one-line edits: `Application::new()` → `gpui_platform::application()`,
a new `Menu.disabled` field, and a `cx` arg on `window.focus()`). In return you get current gpui —
including the wgpu Linux renderer and the `gpui_platform` / `gpui_web` crate split. The **crates.io pair**
(`gpui = "0.2"` + `gpui-component = "0.5"`) stays documented as a plain-stable, zero-git **fallback** —
simpler, but frozen on the Oct-2025 gpui snapshot. Full bump procedure + the fallback swap:
[UPGRADING.md](UPGRADING.md).

What Deck does to stay portable: the only macOS-only code is the tray's dock-hiding
(`setActivationPolicy`), which is `#[cfg(target_os = "macos")]` and whose `objc2` deps are
**target-gated** in `Cargo.toml` (`[target.'cfg(target_os = "macos")'.dependencies]`) so a Linux
`--features tray` build never tries to compile Apple crates. Shortcuts use `secondary` (= ⌘ on macOS,
Ctrl elsewhere) instead of `cmd`, and key-hint chips swap ⌘ for Ctrl via `cfg`. `tray-icon`,
`directories` (XDG vs App Support), and gpui-component's `TitleBar` (traffic lights vs window-control
buttons) all adapt per-OS on their own.

---

## 3. Window chrome & the Linear look

Default `WindowOptions` gives a **standard** titled macOS window
(`titlebar: Some(TitlebarOptions { appears_transparent: false, .. })`). For the modern
frameless/unified look (content under the traffic lights), opt in with the recipe gpui-component
provides:

```rust
titlebar: Some(gpui_component::TitleBar::title_bar_options()),
// → { appears_transparent: true, traffic_light_position: Some(point(9., 9.)), title: None }
```

Then render `TitleBar::new().child(..)` as the **first** child of your root `flex_col`. That's the
whole trick. **Two rules that both panic if violated:** `Root::new(view, ..)` must be the literal top
layer of the window, and `gpui_component::init(cx)` must run before you open any window (it installs
the `Theme` global; otherwise the first `cx.theme()` panics).

---

## 4. Theme — making it not-harsh, with a live accent {#theme}

> *"The default theme is very harsh black and white. What's the common pattern to make it nicer?"*

gpui-component's stock dark theme is near pure-black-on-white. The common pattern (Linear, GitHub,
Zed) is: a **soft** near-black (not `#000`) with slightly-elevated surfaces, **muted** secondary
text, and one saturated **accent** that carries the brand. Three things, really:

1. `background` → a soft near-black with a faint cool cast (`#0C0D11`), not `#000`.
2. `foreground` → a soft white (`#E6E7EB`), not `#FFF`; secondary text via `muted_foreground`.
3. `primary` → a real accent color, not white. This is what makes it feel designed.

### How to apply it (the toggle-safe way)

`Theme::change(mode, ..)` re-applies a `ThemeConfig` on every switch, so don't poke at
`cx.theme().colors` directly — your edits get wiped on the next toggle. Instead **clone the built-in
`ThemeConfig`, override the tokens you care about, and set it as the dark/light config** (`theme.rs`):

```rust
let mut dark = (**ThemeRegistry::global(cx).default_dark_theme()).clone();
dark.colors.background = Some("#0C0D11".into());
dark.colors.foreground = Some("#E6E7EB".into());
dark.colors.primary    = Some(accent_hex.into());      // the brand color
dark.colors.ring       = Some(accent_hex.into());
// …~15 more tokens…
Theme::global_mut(cx).dark_theme = Rc::new(dark);
Theme::change(ThemeMode::Dark, None, cx);              // now re-applies your config
```

`ThemeConfigColors` is ~80 `Option<SharedString>` hex fields (`schema.rs`); unset ones keep the
built-in value. Note the `accent` *token* is a subtle hover **surface**, not the brand — the brand is
`primary`. In a view you read tokens via `cx.theme().<token>` (the `Theme` `Deref`s to its
`ThemeColor`), all `Hsla` you drop straight into `.bg(..)` / `.text_color(..)`.

### Live accent

Because the palette is just a function of one accent color, the settings page can offer a swatch
picker that rebuilds the configs and calls `Theme::change` — the whole app (logo, buttons, focus
rings, even the tray icon) recolors instantly. That's `Shell::set_accent` → `theme::install`.

### The other option: theme JSON files

gpui-component can also hot-load `.theme` JSON files via
`ThemeRegistry::watch_dir(dir, cx, on_load)` (it ships a JSON schema). Great if you want
user-editable / downloadable themes. Deck builds the palette in code instead because it's
self-contained and lets the accent picker mutate it live — but the JSON path is there when you want
it. **Default: dark by default, indigo accent, in-code palette.**

---

## 5. Settings & preference storage {#settings}

> *"Ship a basic preference-saving infrastructure. What's the common, mainstream way? What does Zed
> do? What's the most Rust path?"*

**The mainstream Rust path, and what Deck ships:** a `serde`-derived struct written as JSON
into the OS config directory, found with the [`directories`](https://crates.io/crates/directories)
crate. No database, no framework. The entire layer is ~40 lines (`settings.rs`):

```rust
#[derive(Serialize, Deserialize)]
#[serde(default)]                  // ← missing fields fall back to Default: forward-compatible
struct Settings { theme_mode: ThemeModePref, accent: Accent, display_name: String, /* … */ }

fn path() -> Option<PathBuf> {
    ProjectDirs::from("com", "Example", "Deck")     // reverse-DNS, matches the bundle id
        .map(|d| d.config_dir().join("settings.json"))
}
// load(): read_to_string + serde_json::from_str, fall back to Default on missing/corrupt
// save() -> io::Result: create_dir_all + to_string_pretty + write; save_best_effort() logs & moves on
```

On macOS that resolves to `~/Library/Application Support/com.Example.Deck/settings.json`. Load
once at startup (so the theme reflects saved prefs before the first paint); persist **off the UI hot
path** — at a coarse boundary (blur/commit) or the background executor, never on a per-keystroke
`InputEvent::Change` ([§17](#performance)). The two non-obvious bits: **`#[serde(default)]`** so old
config files survive you adding fields, and **using the platform config dir** (not `~/.myapp`) so
you're a good macOS citizen.

### The spectrum of options

| Approach | What it is | When |
|---|---|---|
| **`directories` + `serde_json`** (Deck) | ~40 lines you own, fully visible | Most apps. Mainstream, zero magic. |
| [`confy`](https://crates.io/crates/confy) | One-liner wrapper over exactly the above (`confy::load`/`store`, TOML by default) | You want it in two lines and don't care where the file is. |
| **Zed's settings system** | Layered **default + user** JSON, file-watched, hot-reloaded, schema-validated, merged into typed structs | A large app with a settings *file* users hand-edit. Overkill for a starter — it lives across several Zed crates, not in gpui. |
| `rusqlite` / a KV store | A real database | Lots of rows (history, documents), queries, migrations. |

**What Zed does**, concretely: it has a dedicated `settings` crate that loads a bundled
`default.json`, merges a user `settings.json` on top, watches both for changes, and deserializes into
typed `Settings` structs via `serde` + `schemars` (the JSON Schema powers editor autocomplete). It's
excellent and it's a lot — the right altitude for an editor users configure by hand, the wrong one
for a fork-and-hack starter. **Default: `directories` + `serde_json`**, with `confy` as the
two-line alternative noted in the code.

---

## 6. Icons — Lucide, already bundled, ISC-licensed {#icons}

> *"Is there a common icon pack we can include with the right license to use freely?"*

**Yes — and it's already here.** gpui-component's `IconName` enum *is*
[Lucide](https://lucide.dev), and `gpui-component-assets` bundles the SVGs (verified: the files are
`arrow-right.svg`, `book-open.svg`, `bot.svg`, … — verbatim Lucide names). **Lucide is ISC-licensed**
— a permissive, MIT-equivalent license, free for personal and commercial use; just keep the
copyright notice if you redistribute the icons (this repo's [NOTICE](../NOTICE) does). So you get a modern ~80-icon set for free.

To make them render you must register the asset source once — `IconName::*` is blank otherwise:

```rust
Application::new().with_assets(gpui_component_assets::Assets).run(..)   // one line, in main.rs
```

Then `Button::new("x").icon(IconName::ArrowRight)` or `Icon::new(IconName::Settings)`.

### Adding your own icons

The bundled set is the subset `IconName` names. For anything else, embed your own SVG and point an
`Icon` at it by path:

```rust
// embed your SVGs in your own AssetSource (rust_embed) under `icons/…`, then:
Icon::empty().path("icons/my-glyph.svg").size_4().text_color(cx.theme().foreground)
```

For the **full** Lucide set (1000+), download the SVGs you want into your assets folder and reference
them the same way. Note: gpui-component bundles icons but **no font** — the UI font is the system
font; to ship a custom typeface you embed the `.ttf` and set `Theme.font_family`.

---

## 7. The app menu bar — native, you just declare it {#menu}

> *"Can we have a menu bar?"* — Yes, fully native.

Build it declaratively and hand it over with `cx.set_menus(Vec<Menu>)`:

```rust
cx.set_menus(vec![
    Menu { name: "Deck".into(), items: vec![
        MenuItem::action("About Deck", About),
        MenuItem::separator(),
        MenuItem::action("Settings…", OpenSettings),     // shows ⌘, automatically
        MenuItem::action("Quit Deck", Quit),         // ⌘Q
    ]},
    Menu { name: "Edit".into(), items: vec![
        MenuItem::os_action("Copy",  NewItem, OsAction::Copy),   // native editing selectors
        MenuItem::os_action("Paste", NewItem, OsAction::Paste),
    ]},
]);
```

The shortcut shown next to an item is derived from your `KeyBinding` for that action. macOS
auto-injects **Services**, **Hide**, and the **Window** items. (`OsAction::{Cut,Copy,Paste,SelectAll}`
wire the native editing selectors; `Undo/Redo` are routed oddly in some GPUI builds — handle them
yourself if needed.)

---

## 8. The dock icon, the tray icon, and tray-first apps {#tray}

> *"Can we have a dock icon? A menu-bar (tray) icon? A tray-first, no-dock app? Does that fit GPUI —
> the tray won't be GPUI-rendered, but I don't want a second rendering system."*

Short version: yes, and it adds no second renderer. The full picture:

### Dock icon — automatic

Every GPUI app is a `Regular` app (`gpui-0.2.2/src/platform/mac/platform.rs:1390` hardcodes
`NSApplicationActivationPolicyRegular`), so it gets a dock icon for free; `cx.activate(true)`
foregrounds it. You **cannot set the dock icon image from Rust** (there's no `setApplicationIconImage`
binding) — the image + label come from the app bundle's `Info.plist`. So `cargo run` shows a generic
icon + the binary name; `cargo bundle` shows yours. Develop with `run`, ship with `bundle`.

### Tray icon — a native status item, **no second renderer**

A macOS menu-bar item (`NSStatusItem`) is, like the dock icon, **just a native image plus a native
menu**. There is nothing to "render" with a UI framework — so adding one introduces **no second
rendering system**. The tray icon is drawn by AppKit; every *window* you show stays 100% GPUI.

GPUI has **no** status-item API itself (neither does Zed), so it's an opt-in. Deck ships it
behind `--features tray` using the [`tray-icon`](https://crates.io/crates/tray-icon) crate (`tray.rs`).
The integration works because **GPUI's run loop *is* the standard AppKit `NSApplication.run` loop**:

```rust
// 1. Build the native status item (image + native menu) on the main thread, in app.run.
let tray = TrayIconBuilder::new().with_icon(brand_icon(accent)).with_menu(..).build()?;
cx.set_global(TrayState { tray });          // keep it alive + reachable for live restyle

// 2. tray-icon posts click/menu events to a global channel. Drain it on GPUI's own
//    executor and act on the main thread — no separate event loop:
cx.spawn(async move |cx| loop {
    cx.background_executor().timer(Duration::from_millis(120)).await;
    while let Ok(ev) = MenuEvent::receiver().try_recv() {
        if ev.id == quit_id { cx.update(|cx| cx.quit())?; }
        if ev.id == show_id { cx.update(|cx| cx.activate(true))?; }
    }
}).detach();
```

Because the icon is a function of the accent, the settings picker restyles it live
(`tray::set_accent` → `TrayIcon::set_icon`). Two rules: create it **on the main thread** inside
`app.run`, and **keep the handle alive** (drop it and the icon vanishes — we stash it in a GPUI
global).

### Tray-first / no-dock apps

To hide the dock icon (a true menu-bar-only app), flip the activation policy to `Accessory` — GPUI
hardcodes `Regular`, so you override it at runtime via objc2 (already in the tree):

```rust
let app = NSApplication::sharedApplication(MainThreadMarker::new()?);
app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);   // no dock icon, no ⌘-Tab
```

That's the whole `--features tray` story: a native status item + accent-synced icon + an
event bridge + one activation-policy call. The window stays GPUI. (For a *true* tray-first app you'd
also skip opening the window at launch and open it on "Show" — a small extension of `tray.rs`.)

---

## 9. App icons — one image in, a `.icns` out

macOS app icons are `.icns` bundles. The pipeline keeps a single source of truth:

```
assets/icon-source.png  ──scripts/make-app-icon.py──▶  assets/icon.png (1024² squircle master)
                                    │                                    │
                              squircle mask                       └─iconutil─▶ assets/icon.icns
                              + pad + shadow
```

`just icon` runs `scripts/make-app-icon.py` — so the *minimum* a forker does is **replace
`assets/icon-source.png`** (any square art: a render, photo, or logo — an `.svg` is rasterized
automatically) and run it. Unlike iOS, macOS does **not** auto-mask icons into the squircle, so the
script bakes the rounded tile, the ~100px inset, and the soft drop shadow into the artwork itself
(true continuous-corner superellipse, supersampled). `--linux` / `--web` also emit a freedesktop
hicolor tree and a full-bleed rounded PNG.

One sharp edge worth knowing: `cargo bundle` 0.11 can build an `.icns` from PNG icons, but a *lone*
1024² PNG trips `No matching IconType` (1024 only exists as 512@2x), so the bundle config points at
the finished `icon.icns` directly — `iconutil` handles every size, including 1024.

The script needs Pillow (`pip install pillow`); everything else is macOS built-ins (`iconutil`). For
a hand-built fallback, `sips -z` each size into an `icon.iconset/` then `iconutil -c icns`.

---

## 10. Bundling — batteries included, not option soup {#bundling}

> *"Is batteries-included right, or should we overload everything with options?"* — **Batteries
> included. Opinionated defaults, escape hatches documented, no option soup.**

The whole bundling story is **one block** in `Cargo.toml`, read by
[`cargo-bundle`](https://github.com/burtonageo/cargo-bundle):

```toml
[package.metadata.bundle]
name = "Deck"
identifier = "com.example.deck"
icon = ["assets/icon.icns"]
category = "public.app-category.productivity"
osx_minimum_system_version = "11.0"
```

`cargo install cargo-bundle` then `just bundle` → a double-clickable `Deck.app`. **Left as opt-in
(documented, not automated)** because they need *your* credentials or aren't needed for local use:
code signing / notarization (a paid Apple Developer ID; `codesign` + `xcrun notarytool`), a custom
`Info.plist`, DMG/installer. Zed uses a big hand-rolled `script/bundle-mac` because it ships to
millions — the wrong altitude for a starter. cargo-bundle's declarative block is the right one.

---

## 11. Keyboard shortcuts & actions

Three lines per shortcut:

```rust
gpui::actions!(deck, [NewItem]);                      // 1. declare (namespace is a bare ident)
cx.bind_keys([KeyBinding::new("cmd-n", NewItem, None)]);  // 2. bind a key
div().on_action(cx.listener(|this, _: &NewItem, _w, cx| { this.count += 1; cx.notify(); }))  // 3. handle
```

Keystroke syntax: `cmd`, `ctrl`, `alt`, `shift`, joined with `-`. Gotchas: **`KeyBinding::new`
panics on a malformed string** (keep them literal); the namespace is a **bare identifier**; **no JSON
keymap loader** exists (you bind in code — typo-checked at compile time); and for a key to reach a
view's `on_action`, the view must be in the focus path (Deck `track_focus`es its root and
focuses it in `Shell::new`).

---

## 12. Components & assets — what's in the box

`gpui-component 0.5.1` modules: `accordion, alert, avatar, badge, breadcrumb, button, chart,
checkbox, … dialog, divider, dock, form, input, kbd, label, link, list, switch, tab, table, tag,
text, tooltip, tree`. Sharp edges that bite forkers:

- **No `Modal`** — the modal primitive is **`Dialog`** (+ a `Sheet` drawer), and overlay layers
  (Dialog/Sheet/Notification) are invisible unless your top view is `Root::new` *and* you emit
  `Root::render_dialog_layer/…`. Deck's pages are plain views to avoid this until you need it.
- **`Input` is stateful** — `Input::new(&entity)` needs an `InputState` *entity* you own and keep
  alive (`cx.new(|cx| InputState::new(window, cx))`); subscribe to `InputEvent::Change` to react.
  (The settings page does exactly this for the display-name field.)
- **Icons need the asset source** (§6); **no bundled font** — UI font is the system font.

---

## 13. Inherited vs. added — the crisp summary

| Inherited from Zed/GPUI + gpui-component (free) | Added by Deck |
|---|---|
| Native window, Metal renderer, GPU text, async executor | Frameless title bar wiring; app bootstrap order |
| Dock icon (automatic); the **menu bar API** | The system menu bar; keyboard shortcuts → actions |
| Theme engine + ~80 tokens; component library | **Refined palette + live accent picker** |
| `actions!` / `KeyBinding` / focus dispatch | **Settings page + JSON preference persistence** |
| Lucide icon set (ISC) | App-icon pipeline + `cargo bundle` config |
| async executor + AppKit run loop | Optional **native tray icon + dock hiding** (`--features tray`) |

---

## 14. Gotcha digest

1. Match the GPUI lineage to gpui-component: **`gpui` 0.2 + `gpui-component` 0.5** (§2).
2. `gpui_component::init(cx)` **before** opening a window, or `cx.theme()` panics.
3. `Root::new(view, ..)` must be the **literal top layer** of the window.
4. Pass `TitleBar::title_bar_options()` for the frameless look (the default titlebar is opaque).
5. `.with_assets(gpui_component_assets::Assets)` or your icons are blank.
6. Don't poke `cx.theme().colors` directly — override the `ThemeConfig` so it survives toggles (§4).
7. Dock icon/label is **bundle-only**; `cargo run` shows the binary name. Ship via `cargo bundle`.
8. Tray icon: create on the main thread in `app.run` and **keep the handle alive** (§8).
9. `KeyBinding::new` panics on bad strings; the `actions!` namespace is a bare ident.
10. No `Modal` (use `Dialog`); `Input` is stateful (`InputState` entity); no bundled font.
11. `List` selection is owned by `ListState` — never set `.selected()` in `render_item` (§16).
12. `.searchable(true)` lives on `ListState`, not the `List` element; `render_initial` *replaces* the
    list on an empty query (§16).

---

## 15. Decisions (chosen → rejected)

- **The matched git pair** — `gpui` + `gpui_platform` (Zed git) + `gpui-component` (Longbridge git),
  pinned via the committed `Cargo.lock`, bumped ~monthly → rejected freezing on the crates.io snapshot
  (now ~8 months stale; kept as a plain-stable, zero-git fallback) and the `gpui-unofficial` tag-mirror
  (not gpui-component-compatible).
- **Soft near-black palette + one live accent** → rejected the harsh stock theme, and rejected a full
  user-editable theme-file system (available via `watch_dir` when you want it).
- **`directories` + `serde_json` for settings** → rejected `confy` (less visible) and a Zed-style
  layered/hot-reloaded settings system (overkill for a starter); rejected a database (no rows yet).
- **Lucide (already bundled, ISC)** → rejected pulling a second icon pack.
- **`cargo bundle` block + a single `icon.png`** → rejected a Zed-style signing/notarizing script.
- **Tray as an opt-in feature, window stays GPUI** → rejected any second rendering system; the status
  item is a native image, nothing more.
- **Command palette = one file on `List`/`ListDelegate`, commands dispatch real actions** → rejected a
  separate registry crate, a bespoke modal, and a heavyweight background fuzzy matcher (§16).
- **A few small files, ~700 lines** → rejected a sprawling, over-engineered monorepo. Read it in ten minutes,
  then start deleting.

---

## 16. The command palette — `List` + `ListDelegate` {#command-palette}

The ⌘K palette (`src/command_palette.rs`) is one file. It leans on the same primitive Zed's own
command palette uses: a **searchable list driven by a delegate**. In gpui-component that's
`ListState<D>` + the `ListDelegate` trait (Zed calls it `Picker` / `PickerDelegate`). You get the
search input, virtualized scrolling, `↑↓` navigation, `↵`/`esc` handling and selection styling for
free; you supply the *rows*, the *match*, and the *chrome*.

**The shape.** A flat `commands()` registry at the top of the file (the one edit surface) → a
`PaletteDelegate` that filters those into grouped `sections` → the `List` renders them. A command
runs by **dispatching a real `gpui` action** (`Run::Action(|| Box::new(OpenSettings))`), so a palette
entry, its hotkey, and its menu item all funnel through the *same* `Shell` handler and can never
drift. The trailing shortcut chip is derived **live from the keymap** via
`Kbd::binding_for_action(&*action, None, window)` — change a `KeyBinding` in `main.rs` and the chip
follows.

**Running a command across the view boundary.** The delegate can't reach `Shell` (its methods only
get `&mut Context<ListState<Self>>`). So `confirm()` just stashes the chosen command in a `pending`
field, and `Shell` — subscribed to the list via `cx.subscribe_in(&palette, window, …)` — drains it on
`ListEvent::Confirm`, **closes the palette first**, then dispatches (so the action lands on `Shell`'s
focus tree, not the palette's). `ListEvent::Cancel` (esc) just closes. This event bridge is the same
pattern the kit uses internally.

**The fuzzy matcher** is ~40 lines, no deps: a subsequence scorer that rewards matches at the start,
at word boundaries (after a space/`-`/`_`) and camelCase humps, and consecutive runs, while penalizing
gaps and length — and returns the matched **byte ranges** so the title can highlight them with
`StyledText::with_highlights`. It runs synchronously: for dozens of commands that's microseconds, so
(unlike Zed) there's no background-threaded matcher or char-bag prefilter. If you grow to thousands of
candidates, that's where you'd add them.

**Custom overlay, not `Dialog`.** The panel is a child the `Shell` renders when open — a scrim
(`background.opacity(0.55)`, correct in both modes) + a `popover`-surfaced panel, anchored near the
top (the Superhuman signature, vs. centered). `gpui-component`'s `Dialog` has title/footer chrome and
its layer only paints if your root view calls `Root::render_dialog_layer` itself — a child overlay is
less wiring and gives full control of position and motion.

### Gotchas that cost real time

- **Don't set `.selected()` in `render_item`** — `ListState` tracks the selected index and applies
  the selection style to whatever item you return. The index you store in the delegate is only there
  so `confirm()` knows what was chosen.
- **`.searchable(true)` is a `ListState` method, not a `List`-element method.** Build it on the state
  (`ListState::new(delegate, …).searchable(true)`); the `List` element only takes `.search_placeholder(…)`.
- **`render_initial` *replaces* the list when the query is empty.** If you want a navigable empty-state
  (recents + all commands), leave `render_initial` as the default `None` and populate the sections in
  `perform_search("")` instead.
- **Unique element ids for duplicated rows.** A command can appear in "Recent" *and* its category, so
  the row id is `(cmd.id, section)` — a bare `cmd.id` would collide and misbehave.
- **Animate opacity, not offset.** The panel sits inside a centering flex (not absolutely
  positioned), so the entrance fades with `.with_animation(.., |el, t| el.opacity(t))`; animating
  `.top()` there would be shaky.
- **Pre-select on open.** A fresh list with an empty query has nothing selected, and `↵` only fires
  when something is selected — so `open_palette` calls `set_selected_index(first_row)` before focusing.

---

## 17. Performance — Linear-esque snappiness, the native way {#performance}

> *"How is [Linear so fast](https://performance.dev/how-is-linear-so-fast-a-technical-breakdown) — and
> which of it applies here?"*

Linear feels instant for **one** reason: the read/write hot path never touches the network. It reads
from an in-memory object graph, applies edits optimistically, and syncs in the background — the
[local-first thesis](https://www.inkandswitch.com/essay/local-first/) (*"there is never a need for the
user to wait for a request to a server to complete"*). Almost everything else people credit — IndexedDB
caching, code-splitting, service-worker precaching, "animate only `transform`/`opacity`," tuning away
React's vdom diff — is **web-platform tax a native binary simply doesn't pay**, and GPUI hands you the
result for free: the heap is your data store, there's no bundle to split, layout is rebuilt each frame
(no reflow to dodge), and clean views are skipped (no vdom to diff). Don't port those tricks. These are
the rules that *do* carry over:

**1. Never block the UI thread on I/O.** Apply the change to in-memory state and `cx.notify()` *now*;
persist *later*, off the hot path. This is the one spot the bare starter originally got wrong: the name
field called `Settings::save()` — a full synchronous `fs::write` of the whole struct — on **every
keystroke** (`InputEvent::Change`). It now mirrors the value into memory on change and persists on
**blur** (`shell.rs`), so the disk never sees the keystroke hot path. `Settings::save()` returns an
`io::Result` for load-bearing writes; `save_best_effort()` is the UI entry point (logs and moves on — a
lost preference must never crash or stall the UI). For a heavier write, **debounce onto the background
executor** instead:

```rust
// store `save_task: Option<Task<()>>` on the view; each keystroke drops the prior
// task (cancelling its timer), coalescing a burst into one write off the render thread:
let settings = self.settings.clone();
self.save_task = Some(cx.spawn(async move |cx: &mut gpui::AsyncApp| {
    cx.background_executor().timer(Duration::from_millis(250)).await;
    cx.background_executor().spawn(async move { settings.save_best_effort() }).await;
}));
```

**2. `cx.notify()` the smallest entity that owns the change.** `notify` marks the view *and all its
ancestors* dirty, so volatile state held as plain fields on `Shell` re-renders the whole page each
tick. Give it its own small `Entity<T>` (as `name_input` and the palette already are) so its churn —
and the root's — stay insulated. GPUI tracks reads per *entity*, not per field, so this is a modeling
choice, not free infrastructure.

**3. Render large collections with `uniform_list` / `list`,** never a flex column of N children — a
naive column rebuilds N elements + N Taffy layout nodes every dirty frame. The virtualized lists render
only the visible window. The ⌘K palette already uses `ListState`; reach for `uniform_list` once a fork
renders real rows.

**4. Filter and search in memory.** The palette matches its registry with a synchronous, dependency-
free fuzzy scorer — no I/O, no background thread per keystroke (§16). Do the same for your own pickers.
