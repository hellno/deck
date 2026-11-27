# LEARNINGS: building a native desktop app on GPUI

How this app is built on GPUI, and the gotchas that cost real time. API claims were checked
against the crates it compiles against: `gpui-kit 0.11` (`gpui-pre 0.3.14`,
`gpui-component 0.11`).

---

## 1. Three layers

```
┌───────────────────────────────────────────────┐
│ your app (src/*.rs)                           │
├───────────────────────────────────────────────┤
│ gpui-component   buttons, inputs, theme,      │  styled component library
│                  TitleBar, Dialog, icons      │
├───────────────────────────────────────────────┤
│ gpui (gpui-pre)  windows, elements, flexbox,  │  Zed's UI framework
│                  renderer, actions, menus     │
└───────────────────────────────────────────────┘
        all through one dependency: gpui-kit
```

GPUI is Zed's GPU-accelerated UI framework: `Render` views hold state and re-render on
`cx.notify()`, layout reads like Tailwind (`div().flex().gap_2().p_4()`), and it ships an
async executor and native plumbing (windows, menu bar, key dispatch). It renders with Metal
on macOS and wgpu/Vulkan on Linux. gpui-component adds the look.

## 2. One dependency: gpui-kit {#dependencies}

`gpui-kit` (Longbridge) depends on a matched set of GPUI crates (`gpui-pre`, published
snapshots of Zed's GPUI), `gpui-base`, `gpui-component` and the default assets. You list
only `gpui-kit`:

```rust
use gpui_kit::*;                       // GPUI
use gpui_kit::component::button::*;    // gpui-component
gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(|cx| { gpui_kit::init(cx); … });
```

Why not depend on GPUI and gpui-component directly: a published gpui-component is built
against one specific GPUI. Mix in a different GPUI and both still compile, but your view
implements one crate's `Render` while `open_window` wants the other's, giving `E0277` at your
call sites. gpui-kit keeps the set matched for you.

**Upgrading:** bump `gpui-kit` (and `rust-version` + `rust-toolchain.toml` when the release
needs a newer compiler; 0.11 needs 1.100), `cargo update -p gpui-kit`, `just ci`, then launch
the app. Some breakage only shows at runtime: `KeyBinding::new("secondary-q", …)` compiles
and panics at launch (§11).

Platform-specific code: the macOS login item, the macOS Dock policy for tray builds (in
`run`), and the tray's GTK thread on Linux (its `gtk` dependency is target-gated).

## 3. Window chrome {#window}

```rust
let options = WindowOptions { window_min_size: …, ..TitleBar::window_options() };
gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Shell::new(…)))?;
```

- `TitleBar::window_options()` gives a transparent native title bar, traffic lights placed
  for gpui-component's 34 px `TitleBar`, and `app_owns_titlebar_drag` so the `TitleBar`
  element handles dragging and double-clicks itself. Render `TitleBar::new()` as the first
  child of your root.
- `gpui_kit::open_window` wraps your view in the Root that paints dialogs, sheets,
  notifications and tooltips. No layer rendering to wire up.
- `gpui_kit::init(cx)` must run before any window opens (`cx.theme()` panics otherwise).
- **Title-bar children drag the window.** The `TitleBar` starts a window move on mouse down
  unless a child calls `window.prevent_default()`. Tabs don't, so clicking a tab moved the
  window. Wrap them: `div().on_mouse_down(MouseButton::Left, |_, w, _| w.prevent_default())`.
- The bar is a fixed 34 px; a default-size segmented `TabBar` fills it. Use `.small()`.

## 4. Theme: looking like a macOS app {#theme}

What made an unthemed app look wrong was the size, not the glyph rendering:
gpui-component's default `font_size` is **16 px** and its Root sets the window's rem from it,
so every `text_*` size is a step too big. A tinted near-black palette made it worse.

`theme.rs` fixes both:

- `ThemeConfig.font_size = Some(13.0)`: the macOS system size.
- Type scale: `text_base` (13 px) for body text, `text_sm` (~11 px) for secondary text, and
  nothing smaller. `text_xs` is under 10 px.
- Neutral greys close to AppKit's for light and dark, and one accent (`#0064E1`, dark enough
  for white text at 5.4:1; macOS's `#007AFF` is 4.0:1). A test checks AA contrast for text
  and accent in both modes.
- Appearance: System / Light / Dark. "System" uses `Theme::sync_system_appearance` and
  follows changes through `cx.observe_window_appearance`.

**How to apply a palette so it survives mode switches:** `Theme::change` re-applies a
`ThemeConfig` every time, so don't edit `cx.theme()` colours directly. Clone the registry's
default configs, override what you need, and install them as the light/dark configs:

```rust
let mut light = (**ThemeRegistry::global(cx).default_light_theme()).clone();
light.font_size = Some(13.0);
light.colors.background = Some("#F5F5F5".into());
Theme::global_mut(cx).light_theme = Rc::new(light);
Theme::change(ThemeMode::Light, None, cx);
```

Unset tokens keep the built-in value. gpui-component's `accent` token is a hover fill, not the
brand colour; the brand is `primary`.

Component colours that need setting explicitly:

- **`tab_bar_segmented`**: the segmented control's selected segment is drawn in `background`,
  so the track must contrast with the window. In dark mode that means a *darker* track.
- **`switch_thumb`**: set it (white) or the thumb can disappear against the track.

## 5. Settings and storage {#settings}

A `serde` struct written as JSON into the OS config directory (via `directories`):
`~/Library/Application Support/<bundle id>/settings.json` on macOS, XDG on Linux.

- `#[serde(default)]` on the struct: old files keep loading as fields come and go.
- **An unreadable file is moved aside** (`settings.json.unreadable-<unix time>`) before
  falling back to defaults, and the Settings page shows an `Alert`. A loader that silently
  falls back lets the next save destroy the user's file. If even the move fails, saving is
  off for the session.
- A dry run (`APP_DRY_RUN=1`) uses `settings.dry-run.json`.
- Writes are atomic (temp file + rename) and stay off the hot path (§13).

| Approach | When |
|---|---|
| `directories` + `serde_json` (this) | Most apps. ~100 lines you can read. |
| [`confy`](https://crates.io/crates/confy) | The same in two lines, less visible. |
| Zed's layered, file-watched settings | Editors whose users hand-edit settings. Overkill here. |
| `rusqlite` or a KV store | Many rows, queries, migrations. |

## 6. Icons {#icons}

gpui-component's `IconName` is [Lucide](https://lucide.dev) (ISC, permissive), bundled by
`gpui-kit-assets`. Register the asset source once or icons render blank:
`application().with_assets(gpui_kit::assets::Assets)`. Keep `NOTICE` if you redistribute.
For other SVGs, serve them from your own `AssetSource` and use `Icon::empty().path("icons/x.svg")`.
The UI font is the system font.

## 7. The menu bar {#menu}

Declare it with `cx.set_menus(Vec<Menu>)` (`src/lib.rs`). The shortcut shown next to an item
comes from your key bindings. `MenuItem::os_action(…, OsAction::Copy)` wires the native
editing selectors. A `Window` menu and a Services item exist only if you declare them (a
`Menu` named "Window", a `MenuItem::os_submenu("Services", SystemMenuType::Services)`).

## 8. Dock, tray, and menu-bar apps {#tray}

- **Dock icon:** automatic. Its image and name come from the bundle's `Info.plist`, so
  `cargo run` shows a generic icon; `just bundle` shows yours.
- **Tray icon** (`--features tray`, `tray-icon` crate): a native status item, an image plus a
  native menu. No second renderer; windows stay GPUI.
- **Events without polling:** `MenuEvent::set_event_handler` sends each event into a channel
  (`gpui_kit::base::async_util::unbounded`); a `cx.spawn` task awaits it on the main thread.
- **Quit mode:** GPUI's default quits when the last window closes on Linux (not macOS). A tray
  app sets `cx.set_quit_mode(QuitMode::Explicit)`.
- **"Show" reopens a closed window**, and tray handlers **call functions**, not actions: an
  action dispatched with no focused window reaches nothing.
- **No Dock icon:** the `Accessory` activation policy, set **before launch**:
  `application().with_windowing(WindowingRequest::Headless)`, on macOS only (on Linux,
  headless means no display server). Inside `run`, GPUI has already applied `Regular`, so
  `cx.set_activation_policy(ActivationPolicy::Accessory)` there works but flashes the Dock icon
  for about 0.1 s. Use `set_activation_policy` to switch at runtime, and call `cx.activate(true)`
  after switching back to `Regular`, or the menu bar may not appear. Don't call AppKit's
  `setActivationPolicy` through `objc2`: GPUI keeps its own record of the policy, which then goes
  stale. Check with `lsappinfo info -only applicationtype <pid>`: `UIElement` means no Dock
  icon, `Foreground` means a Dock icon.
- **Where the icon lives:** on macOS, on the main thread inside `run`, kept alive in a GPUI
  global. On Linux, tray-icon needs a GTK event loop on the icon's thread and GPUI runs its
  own loop, so the icon gets a thread that calls `gtk::init()` and `gtk::main()`.

## 9. App icons

```
assets/icon-source.png ─ scripts/make-app-icon.py ─▶ assets/icon.png ─ iconutil ─▶ assets/icon.icns
```

`just icon` bakes the macOS squircle, inset and shadow into the artwork (macOS doesn't mask
app icons). `just icon-linux` also writes a freedesktop hicolor tree and a `.desktop` file
under `dist/linux`. Needs Pillow. The bundle config points at the finished `.icns`.

## 10. Bundling, installing, login items {#bundling}

`cargo-bundle` reads `[package.metadata.bundle]`. In 0.12 the minimum macOS version lives in
a sub-table; a top-level `osx_minimum_system_version` is rejected as an unknown field:

```toml
[package.metadata.bundle.macos]
minimum_system_version = "11.0"
```

- `just bundle` runs `cargo bundle --release --format osx`. Without `--format`, 0.12 also
  tries a DMG and fails if `hdiutil` can't mount.
- `just install` signs ad hoc (`codesign --force --deep -s -`), quits the running copy and
  replaces it in `/Applications`. It refuses to replace an app with a different bundle id, and
  keeps the old copy until the new one is in place. macOS notifications need the app launched
  from a bundle.
- **Open at login** (`src/login_item.rs`, macOS): a LaunchAgent plist in
  `~/Library/LaunchAgents`. Offered only when the app runs from `/Applications` or
  `~/Applications`: a login item pointing at `target/debug` breaks on the next build. To remove
  it, delete the module, its `mod` line in `lib.rs`, the `login_item` field in `Shell` and
  `render_login_item` in `settings_view.rs`.
- Signing with a Developer ID and notarizing stay manual (`codesign`, `xcrun notarytool`).

## 11. Shortcuts and actions

```rust
gpui_kit::actions!(my_app, [NewItem]);                               // declare
("secondary-n", Box::new(NewItem))                                   // list in shortcuts()
KeyBinding::load(keys, action, None, false, None, cx)?               // bind at launch
div().on_action(cx.listener(|this, _: &NewItem, window, cx| …))      // handle
```

- `secondary` means ⌘ on macOS and Ctrl elsewhere, but only `KeyBinding::load` resolves it
  (with the host OS). **`KeyBinding::new` parses without an OS and panics on `secondary-` at
  launch.** `shortcuts()` returns every binding, and a test parses each for macOS and Linux
  with `Keystroke::parse_with_os`.
- An action reaches a view's `on_action` only if the view is in the focus path; the root
  view `track_focus`es itself and takes focus on creation.

## 12. Components {#components}

Build the UI from gpui-component rather than hand-drawn look-alikes:

| Need | Component | Notes |
|---|---|---|
| Page switcher | `TabBar::new(..).segmented().small()` | wrap in `prevent_default` inside a `TitleBar` (§3) |
| Settings groups | `GroupBox::new().fill()` (`ui::group`) | `ui::row` rows, `Separator::horizontal()` between them |
| Choices | `Select` + `SelectState` | wrap it in a fixed-width `flex_none` div or it stretches |
| Toggles | `Switch` | set `switch_thumb` (§4); disable with the `Disableable` trait |
| Nothing to show | `Empty` with header/media/title/description | |
| Warnings | `Alert::warning(id, message)` | |
| Modal input | `window.open_dialog(cx, …)` with `v_form().child(field().label(..).child(Input::new(&state)))` | return `false` from `on_ok` to keep it open |
| About box | `window.prompt(PromptLevel::Info, …)` | a native NSAlert on macOS |
| Command palette | `Command` + `CommandState` (`palette.rs`) | rows take the menu's actions; render it inside the view that handles them |

Sharp edges:

- `Field` descriptions default to `text_xs` (under 10 px at a 13 px base). Pass your own
  `text_sm` element.
- gpui-component's `setting` module has row renderers, but they're crate-private; `ui.rs`
  covers the same ground in ~50 lines.
- `Input` is stateful: keep the `InputState` entity alive and read `state.read(cx).value()`.
- A palette row's action is dispatched from the palette's focus, so it bubbles only through
  the palette's ancestors. A `Dialog` is rendered by `Root`, outside `Shell`, so a palette in
  a dialog never reaches Shell's handlers. Render it as an overlay inside `Shell`.
- A dialog returns focus to whatever was focused when it opened. A command that opens one
  from the palette (New Item) closes the palette first, or focus goes to a palette that's gone.
- Dialogs animate in. Anything that captures a frame (`just shot`) must advance the clock
  first, or it captures the dialog's first, invisible frame.

## 13. Performance {#performance}

A native GPUI app gets most of "why is Linear fast" for free: the heap is your data store,
there's no bundle to split, and clean views are skipped. The rules that still need care:

1. **Never block the UI thread on I/O.** Update memory and `cx.notify()` now; persist at a
   coarse boundary (blur, commit) or on `cx.background_executor()`. For bursts, debounce:

   ```rust
   // `save_task: Option<Task<()>>` on the view; replacing it cancels the previous timer.
   self.save_task = Some(cx.spawn(async move |_, cx| {
       cx.background_executor().timer(Duration::from_millis(250)).await;
       // write here (or hand the write to background_executor().spawn)
   }));
   ```
2. **Notify the smallest entity that changed.** `cx.notify()` dirties the view and its
   ancestors; volatile state belongs in its own `Entity<T>`, not on the root view.
3. **Long lists use `uniform_list` / `list`**, never a flex column of N children.
4. **Filter and search in memory.**

## 14. Letting agents see and drive the app {#agents}

Driving the real app from an agent goes wrong in predictable ways:

- A normal launch activates the app (`cx.activate(true)`), so the user's clicks land in the
  agent's window.
- A covered GPUI window stops repainting (its display link stops when occluded), so screen
  captures go stale.
- Accessibility queries can have side effects (expanding a list, for one).
- Global keystrokes land in whatever app is frontmost: the user's terminal, if the app
  crashed at launch.

What this project does instead:

- **`just shot`** (`examples/shot.rs`): `HeadlessAppContext` with the real text system and the
  headless Metal/wgpu renderer renders the real root view offscreen. No window, no Dock icon,
  no focus change; a 2x PNG that's byte-identical across runs. Limits: the view only (no
  traffic lights), fixed 2x, and it needs the GPU, so it fails in sandboxes like Seatbelt.
  It needs `gpui-kit`'s `test-support` feature, which is a dev-dependency only (it pulls in
  proptest and friends).
- **Env hooks** (`src/launch.rs`) put the app in a state without clicks: `APP_PAGE`,
  `APP_APPEARANCE`, `APP_FIXTURES`. Add your own (a fake network state, a data set).
- **Dry run** (`APP_DRY_RUN=1`): separate settings file, no activation or focus, nothing
  irreversible.
- **Tests drive the real views:** `#[gpui_kit::test]` opens the root view in a test window,
  where `press`, `input` and `click` go through the real keymap and focus (`src/shell.rs`).
- **Fail loudly on unknown flags.** A stale binary that predates a flag would otherwise launch
  the full app. `main.rs` exits 2 on unknown arguments; `just ci` builds the binary.
