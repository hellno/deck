//! Command palette — the ⌘K (Ctrl K) launcher, Superhuman / Linear style.
//!
//! A floating, top-anchored panel over a soft scrim: type to fuzzy-search every
//! command, ↑↓ to move, ↵ to run, esc to close. It's built on gpui-component's
//! `List` + `ListDelegate` (the same searchable-list primitive Zed's own palette
//! uses), so the search box, keyboard navigation, scrolling and selection styling
//! all come for free — this file only supplies the *commands*, the *fuzzy match*,
//! and the *chrome*.
//!
//! ## Add a command (the whole story)
//!
//! Add one line to `commands()` below. If it maps to an action you already have
//! (the common case), point `run` at it and you're done — the palette dispatches
//! the *same* action as the hotkey and the menu bar, so behaviour can never drift,
//! and the trailing shortcut chip is derived live from your keymap:
//!
//! ```ignore
//! Command { id: "home", title: "Go Home", icon: IconName::ArrowLeft,
//!           category: Category::Navigation, keywords: &["back", "welcome"],
//!           run: Run::Action(|| Box::new(GoBack)) },
//! ```
//!
//! For a palette-only verb with no standing action, use the `Run::Accent`-style
//! escape hatch (or add your own `Run` variant + a match arm in `run_command`).
//!
//! ## How it's wired (see `shell.rs` + `main.rs`)
//!
//! `main.rs` declares the `TogglePalette` action and binds `secondary-k` to it.
//! `Shell` owns the open/closed state, builds a fresh `ListState` each time the
//! palette opens (so the query always starts empty), and renders the overlay as
//! its last child. Because the delegate can't reach `Shell`, running a command is
//! bridged through the list's `ListEvent`: the delegate stashes the chosen command
//! in `pending`, and `Shell::on_palette_event` drains it, closes the palette, then
//! dispatches (see the `impl Shell` block at the bottom of this file).
//!
//! ## Extending
//!
//! - **A verb with no action / with data.** Add a `Run` variant (e.g.
//!   `Run::OpenUrl(&'static str)`) and a match arm in `Shell::run_command`. Keep the
//!   `Run` match in `render_item` (the shortcut-chip derivation) in sync too.
//! - **A new group.** Add a `Category` variant and list it in `Category::ORDER`.
//! - **Persistent recents.** Recents are session-only by design (no disk I/O on the
//!   hot path). To survive restarts: add `recents: Vec<String>` to `Settings`, load it
//!   in `Shell::new`, and call `self.settings.save()` from `record_recent` — the same
//!   pattern `settings.rs` already uses for `display_name`.

use std::ops::Range;
use std::time::Duration;

use gpui::{
    div, ease_out_quint, hsla, px, Animation, AnimationExt, AppContext, Context, Entity,
    FontWeight, HighlightStyle, InteractiveElement, IntoElement, MouseButton, ParentElement,
    Styled, StyledText, Task, Window,
};
use gpui_component::{
    h_flex,
    kbd::Kbd,
    list::{List, ListDelegate, ListEvent, ListItem, ListState},
    v_flex, ActiveTheme, Icon, IconName, IndexPath,
};

use crate::shell::Shell;
use crate::theme::Accent;
use crate::{GoBack, NewItem, OpenSettings, Quit, TogglePalette, ToggleTheme};

// ===========================================================================
// 1. The command registry — THE place you edit to add/remove commands.
// ===========================================================================

/// A group heading in the palette. The order here is the order on screen.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Navigation,
    Create,
    Appearance,
    App,
}

impl Category {
    /// Sections render in this order; empty groups are dropped automatically.
    const ORDER: [Category; 4] = [
        Category::Navigation,
        Category::Create,
        Category::Appearance,
        Category::App,
    ];

    fn label(self) -> &'static str {
        match self {
            Category::Navigation => "Navigation",
            Category::Create => "Create",
            Category::Appearance => "Appearance",
            Category::App => "App",
        }
    }
}

/// What running a command does.
#[derive(Clone, Copy)]
pub enum Run {
    /// Dispatch a real gpui action. This funnels through the *same* handler as
    /// the keyboard shortcut and the native menu item, so the three can never
    /// disagree — and a command that reuses an existing action needs no new code.
    Action(fn() -> Box<dyn gpui::Action>),
    /// Set the brand accent. An example of a palette-only verb that has no
    /// standing action/hotkey — handled directly in `Shell` (see `run_command`).
    Accent(Accent),
}

/// One palette entry. (`Clone`, not `Copy`, because `IconName` isn't `Copy`.)
#[derive(Clone)]
pub struct Command {
    /// Stable id — used as the recents key and part of the row's element id.
    pub id: &'static str,
    /// The label you see (also what fuzzy-search matches + highlights).
    pub title: &'static str,
    /// Leading glyph (Lucide; see `IconName`).
    pub icon: IconName,
    /// Which section it lives in.
    pub category: Category,
    /// Extra search aliases that don't appear in the title (e.g. "prefs").
    pub keywords: &'static [&'static str],
    /// What it does.
    pub run: Run,
}

/// The command set Deck ships with. **Add your commands here.** Everything maps
/// to an action `Shell`/`main.rs` already handles, except the six accent verbs,
/// which show off the `Run::Accent` escape hatch.
pub fn commands() -> Vec<Command> {
    let mut cmds = vec![
        Command {
            id: "settings",
            title: "Open Settings",
            icon: IconName::Settings,
            category: Category::Navigation,
            keywords: &["preferences", "prefs", "config"],
            run: Run::Action(|| Box::new(OpenSettings)),
        },
        Command {
            id: "home",
            title: "Go Home",
            icon: IconName::ArrowLeft,
            category: Category::Navigation,
            keywords: &["back", "welcome"],
            run: Run::Action(|| Box::new(GoBack)),
        },
        Command {
            id: "new",
            title: "New Item",
            icon: IconName::Plus,
            category: Category::Create,
            keywords: &["add", "create"],
            run: Run::Action(|| Box::new(NewItem)),
        },
        Command {
            id: "theme",
            title: "Toggle Theme",
            icon: IconName::Sun,
            category: Category::Appearance,
            keywords: &["dark", "light", "mode", "appearance"],
            run: Run::Action(|| Box::new(ToggleTheme)),
        },
    ];

    // Six accent commands, generated from `Accent::ALL` (mirrors settings_view.rs).
    cmds.extend(Accent::ALL.iter().map(|&accent| {
        let (id, title) = accent_meta(accent);
        Command {
            id,
            title,
            icon: IconName::Palette,
            category: Category::Appearance,
            keywords: &["color", "accent", "brand"],
            run: Run::Accent(accent),
        }
    }));

    cmds.push(Command {
        id: "quit",
        title: "Quit",
        icon: IconName::Close,
        category: Category::App,
        keywords: &["exit", "close"],
        run: Run::Action(|| Box::new(Quit)),
    });

    cmds
}

/// `(stable id, palette title)` for an accent — kept here so `theme.rs` stays
/// purely about colour.
fn accent_meta(accent: Accent) -> (&'static str, &'static str) {
    match accent {
        Accent::Indigo => ("accent-indigo", "Accent · Indigo"),
        Accent::Blue => ("accent-blue", "Accent · Blue"),
        Accent::Violet => ("accent-violet", "Accent · Violet"),
        Accent::Emerald => ("accent-emerald", "Accent · Emerald"),
        Accent::Amber => ("accent-amber", "Accent · Amber"),
        Accent::Rose => ("accent-rose", "Accent · Rose"),
    }
}

// ===========================================================================
// 2. The fuzzy matcher.
// ===========================================================================
//
// A compact subsequence scorer — every query char must appear in order, and we
// reward matches at the start, at word boundaries (after a space / - / _) and
// camelCase humps, and consecutive runs, while penalising gaps and length. It
// returns the matched byte ranges so we can highlight them. This is a small,
// readable cousin of Zed's `fuzzy` crate; for a palette of dozens of commands
// it runs synchronously in microseconds — no background threads needed.

/// Fuzzy-match `query` against `haystack`. `None` if not a subsequence; otherwise
/// `Some((score, matched byte ranges))`, higher score = better.
fn fuzzy(query: &str, haystack: &str) -> Option<(i32, Vec<Range<usize>>)> {
    if query.is_empty() {
        return Some((0, Vec::new()));
    }

    let needles: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    let mut ni = 0usize;
    let mut ranges: Vec<Range<usize>> = Vec::with_capacity(needles.len());
    let mut score = 0i32;
    // Two cursors so multi-byte UTF-8 is handled correctly: `byte` is the real
    // string offset we hand back for highlight ranges; `char_ix` counts logical
    // characters, which is what adjacency/gap scoring should reason about.
    let mut prev_match: i64 = -2;
    let mut prev: Option<char> = None;

    for (char_ix, (byte, ch)) in (0i64..).zip(haystack.char_indices()) {
        if ni == needles.len() {
            break;
        }
        let lc = ch.to_lowercase().next().unwrap_or(ch);
        if lc == needles[ni] {
            if char_ix == 0 {
                score += 15; // start of string
            } else if matches!(prev, Some(p) if p == ' ' || p == '-' || p == '_' || p == '/') {
                score += 12; // word boundary
            } else if matches!(prev, Some(p) if p.is_lowercase()) && ch.is_uppercase() {
                score += 9; // camelCase hump
            }
            if char_ix == prev_match + 1 {
                score += 6; // consecutive run
            } else {
                score -= ((char_ix - prev_match - 1).min(6)) as i32; // gap penalty (capped)
            }
            ranges.push(byte..byte + ch.len_utf8());
            prev_match = char_ix;
            ni += 1;
        }
        prev = Some(ch);
    }

    if ni == needles.len() {
        score -= haystack.chars().count() as i32 / 4; // mild preference for shorter labels
        Some((score, ranges))
    } else {
        None
    }
}

/// Score a command: a title match (which carries highlight ranges) wins; failing
/// that, a keyword match counts but ranks a little lower and highlights nothing.
fn score_command(query: &str, cmd: &Command) -> Option<(i32, Vec<Range<usize>>)> {
    if let Some(hit) = fuzzy(query, cmd.title) {
        return Some(hit);
    }
    let mut best: Option<i32> = None;
    for kw in cmd.keywords {
        if let Some((s, _)) = fuzzy(query, kw) {
            best = Some(best.map_or(s, |b| b.max(s)));
        }
    }
    best.map(|s| (s - 4, Vec::new()))
}

// ===========================================================================
// 3. The list delegate — feeds rows to gpui-component's `List`.
// ===========================================================================

/// A matched row: a command plus the byte ranges of its title to highlight.
#[derive(Clone)]
struct Hit {
    cmd: Command,
    ranges: Vec<Range<usize>>,
}

impl Hit {
    /// A row with no highlighting (used for the empty-query / recents view).
    fn plain(cmd: Command) -> Self {
        Hit {
            cmd,
            ranges: Vec::new(),
        }
    }
}

/// A visible group: a heading and its rows.
struct Section {
    title: &'static str,
    rows: Vec<Hit>,
}

/// The data + filtering behind the palette. The `List` owns the search input,
/// keyboard navigation and selection; this just answers "what rows, in what
/// groups?" and remembers which command was confirmed.
pub struct PaletteDelegate {
    all: Vec<Command>,
    /// Snapshot of recently-run ids, passed in by `Shell` when the palette opens.
    recents: Vec<&'static str>,
    sections: Vec<Section>,
    /// The currently highlighted row; the `List` keeps this in sync. Read in
    /// `confirm` to know what to run.
    selected: Option<IndexPath>,
    /// Set by `confirm`, drained by `Shell::on_palette_event`.
    pending: Option<(Run, &'static str)>,
}

impl PaletteDelegate {
    pub fn new(recents: Vec<&'static str>) -> Self {
        let mut this = Self {
            all: commands(),
            recents,
            sections: Vec::new(),
            selected: None,
            pending: None,
        };
        this.rebuild("");
        this
    }

    fn resolve(&self, id: &str) -> Option<Command> {
        self.all.iter().find(|c| c.id == id).cloned()
    }

    /// The first selectable row, used to pre-select something on open.
    pub fn first_row(&self) -> Option<IndexPath> {
        self.sections
            .iter()
            .position(|s| !s.rows.is_empty())
            .map(|s| IndexPath::new(0).section(s))
    }

    /// Take the command confirmed by the user, if any.
    pub fn take_pending(&mut self) -> Option<(Run, &'static str)> {
        self.pending.take()
    }

    /// Recompute the visible sections for `query`.
    fn rebuild(&mut self, query: &str) {
        let query = query.trim();
        self.sections.clear();

        if query.is_empty() {
            // Empty query: a "Recent" group, then every *other* command grouped by
            // category. Recent commands lift out of their group (Raycast/Spotlight
            // style) so nothing appears twice.
            let recent: Vec<Command> = self
                .recents
                .iter()
                .filter_map(|id| self.resolve(id))
                .collect();
            if !recent.is_empty() {
                self.sections.push(Section {
                    title: "Recent",
                    rows: recent.iter().map(|cmd| Hit::plain(cmd.clone())).collect(),
                });
            }
            for cat in Category::ORDER {
                let rows: Vec<Hit> = self
                    .all
                    .iter()
                    .filter(|c| c.category == cat && !recent.iter().any(|r| r.id == c.id))
                    .map(|cmd| Hit::plain(cmd.clone()))
                    .collect();
                if !rows.is_empty() {
                    self.sections.push(Section {
                        title: cat.label(),
                        rows,
                    });
                }
            }
        } else {
            // Non-empty query: each category's matches, best score first.
            for cat in Category::ORDER {
                let mut scored: Vec<(i32, Hit)> = self
                    .all
                    .iter()
                    .filter(|c| c.category == cat)
                    .filter_map(|cmd| {
                        score_command(query, cmd).map(|(s, ranges)| {
                            (
                                s,
                                Hit {
                                    cmd: cmd.clone(),
                                    ranges,
                                },
                            )
                        })
                    })
                    .collect();
                scored.sort_by_key(|entry| std::cmp::Reverse(entry.0));
                if !scored.is_empty() {
                    self.sections.push(Section {
                        title: cat.label(),
                        rows: scored.into_iter().map(|(_, hit)| hit).collect(),
                    });
                }
            }
        }
    }
}

impl ListDelegate for PaletteDelegate {
    type Item = ListItem;

    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.rebuild(query);
        Task::ready(())
    }

    fn sections_count(&self, _: &gpui::App) -> usize {
        self.sections.len()
    }

    fn items_count(&self, section: usize, _: &gpui::App) -> usize {
        self.sections.get(section).map_or(0, |s| s.rows.len())
    }

    fn render_section_header(
        &mut self,
        section: usize,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<impl IntoElement> {
        let title = self.sections.get(section)?.title;
        Some(
            div()
                .px_3()
                .pt_3()
                .pb_1()
                .text_xs()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(cx.theme().muted_foreground)
                .child(title.to_uppercase()),
        )
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        window: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let hit = self.sections.get(ix.section)?.rows.get(ix.row)?;
        let cmd = &hit.cmd;
        let ranges = hit.ranges.clone();
        // The List applies the selected *background*; we only use this to brighten
        // the leading icon on the selected row (it stays quiet on the rest).
        let is_selected = self.selected == Some(ix);

        // Derive the shortcut chip live from the keymap, so it always mirrors the
        // real binding in main.rs (None = global context, which is how Deck binds).
        let kbd = match cmd.run {
            Run::Action(make) => Kbd::binding_for_action(&*make(), None, window),
            Run::Accent(_) => None,
        };

        let foreground = cx.theme().foreground;
        let muted = cx.theme().muted_foreground;
        let accent = cx.theme().primary;
        let icon_color = if is_selected { foreground } else { muted };
        let title = cmd.title;

        // The title, with matched characters tinted + bolded. `flex_1().truncate()`
        // keeps long titles from shoving the shortcut chip off the row.
        let label = {
            let base = div().flex_1().truncate().text_sm().text_color(foreground);
            if ranges.is_empty() {
                base.child(title)
            } else {
                let highlights = ranges.into_iter().map(move |r| {
                    (
                        r,
                        HighlightStyle {
                            color: Some(accent),
                            font_weight: Some(FontWeight::SEMIBOLD),
                            ..Default::default()
                        },
                    )
                });
                base.child(StyledText::new(title).with_highlights(highlights))
            }
        };

        // NOTE: do NOT call `.selected(..)` here — the `List` applies selection
        // styling itself from the index it tracks. The id pairs the command with
        // its section so the same command can appear in "Recent" and its category
        // without colliding.
        Some(
            ListItem::new((cmd.id, ix.section))
                .suffix(move |_, _| match kbd.clone() {
                    Some(k) => k.into_any_element(),
                    None => div().into_any_element(),
                })
                .child(
                    h_flex()
                        .w_full()
                        .gap_3()
                        .items_center()
                        .child(Icon::new(cmd.icon.clone()).size_4().text_color(icon_color))
                        .child(label),
                ),
        )
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        v_flex()
            .py_8()
            .gap_2()
            .items_center()
            .child(
                Icon::new(IconName::Search)
                    .size_6()
                    .text_color(cx.theme().muted_foreground.opacity(0.5)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("No matching commands"),
            )
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.selected = ix;
    }

    fn confirm(&mut self, _: bool, _: &mut Window, _: &mut Context<ListState<Self>>) {
        // The List only calls this when something is selected, but fall back to
        // the first row defensively.
        let ix = self.selected.or_else(|| self.first_row());
        if let Some(ix) = ix {
            if let Some(hit) = self
                .sections
                .get(ix.section)
                .and_then(|s| s.rows.get(ix.row))
            {
                self.pending = Some((hit.cmd.run, hit.cmd.id));
            }
        }
    }
}

// ===========================================================================
// 4. Shell integration — open/close, render the overlay, run commands.
//    (These are `Shell` methods, split into this module like welcome.rs /
//    settings_view.rs do; the fields live in `shell.rs`.)
// ===========================================================================

/// How far the panel floats from the top of the window.
const TOP_OFFSET: f32 = 112.0;
const PANEL_WIDTH: f32 = 620.0;

impl Shell {
    /// ⌘K — toggle the palette. Bound in `main.rs`, wired in `shell.rs`'s render.
    pub fn on_palette_toggle(
        &mut self,
        _: &TogglePalette,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.palette.is_some() {
            self.close_palette(window, cx);
        } else {
            self.open_palette(window, cx);
        }
    }

    /// Build a fresh palette (so the query always starts empty), pre-select the
    /// first row, and focus the search input.
    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let recents = self.recents.iter().copied().collect::<Vec<_>>();
        let palette =
            cx.new(|cx| ListState::new(PaletteDelegate::new(recents), window, cx).searchable(true));
        self.palette_sub = Some(cx.subscribe_in(&palette, window, Self::on_palette_event));
        palette.update(cx, |state, cx| {
            let first = state.delegate().first_row();
            state.set_selected_index(first, window, cx);
            state.focus(window, cx);
        });
        self.palette = Some(palette);
        cx.notify();
    }

    /// Close the palette and return focus to the shell so global hotkeys work.
    pub fn close_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.palette = None;
        self.palette_sub = None;
        window.focus(&self.focus_handle);
        cx.notify();
    }

    /// Bridge the list's events back to the shell: run on confirm, close on cancel.
    pub fn on_palette_event(
        &mut self,
        state: &Entity<ListState<PaletteDelegate>>,
        event: &ListEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event {
            ListEvent::Confirm(_) => {
                let chosen = state.update(cx, |s, _| s.delegate_mut().take_pending());
                // Close first, so the dispatched action lands on the shell's focus
                // tree (not the palette's).
                self.close_palette(window, cx);
                if let Some((run, id)) = chosen {
                    self.record_recent(id);
                    self.run_command(run, window, cx);
                }
            }
            ListEvent::Cancel => self.close_palette(window, cx),
            ListEvent::Select(_) => {}
        }
    }

    fn record_recent(&mut self, id: &'static str) {
        self.recents.retain(|r| *r != id);
        self.recents.push_front(id);
        self.recents.truncate(8);
    }

    fn run_command(&mut self, run: Run, window: &mut Window, cx: &mut Context<Self>) {
        match run {
            Run::Action(make) => window.dispatch_action(make(), cx),
            Run::Accent(accent) => self.set_accent(accent, cx),
        }
    }

    /// The overlay: a scrim with the floating panel, anchored near the top.
    pub fn render_palette(
        &self,
        palette: &Entity<ListState<PaletteDelegate>>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let border = cx.theme().border;

        div()
            .id("palette-scrim")
            .absolute()
            .inset_0()
            .flex()
            .flex_col()
            .items_center()
            .justify_start()
            .pt(px(TOP_OFFSET))
            // A soft dark scrim that *darkens* behind the panel in BOTH modes (a
            // theme `background` tint would wash out white in light mode). Kept a
            // local constant — a scrim is conventionally mode-independent.
            .bg(hsla(0.0, 0.0, 0.0, 0.45))
            // Click outside the panel to dismiss.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.close_palette(window, cx)),
            )
            .child(
                v_flex()
                    .w(px(PANEL_WIDTH))
                    .max_w_full()
                    .max_h(px(440.0))
                    .rounded_xl()
                    .border_1()
                    .border_color(border)
                    .bg(cx.theme().popover)
                    .shadow_lg()
                    .overflow_hidden()
                    // Swallow clicks inside the panel so they don't hit the scrim.
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        List::new(palette)
                            .search_placeholder("Search commands…")
                            .max_h(px(360.0)),
                    )
                    .child(self.render_palette_footer(cx))
                    // Subtle, fast entrance — opacity only (the panel isn't
                    // absolutely positioned, so animating offset would be shaky).
                    .with_animation(
                        "palette-in",
                        Animation::new(Duration::from_millis(120)).with_easing(ease_out_quint()),
                        |el, t| el.opacity(t),
                    ),
            )
    }

    /// The whisper-thin hint bar at the bottom of the panel. Delete this method's
    /// call in `render_palette` if you want it even quieter.
    fn render_palette_footer(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let border = cx.theme().border;
        let muted = cx.theme().muted_foreground;
        let surface = cx.theme().secondary;

        let chip = move |key: &str| {
            div()
                .px_1()
                .rounded_sm()
                .border_1()
                .border_color(border)
                .bg(surface)
                .text_color(muted)
                .child(key.to_string())
        };
        let hint = move |keys: Vec<&'static str>, label: &'static str| {
            h_flex()
                .gap_1()
                .items_center()
                .children(keys.into_iter().map(chip))
                .child(div().child(label))
        };

        h_flex()
            .w_full()
            .px_3()
            .py_1p5()
            .gap_3()
            .items_center()
            .border_t_1()
            .border_color(border)
            .text_xs()
            .text_color(muted)
            .child(hint(vec!["↑", "↓"], "Navigate"))
            .child(hint(vec!["↵"], "Open"))
            .child(hint(vec!["esc"], "Close"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(q: &str, hay: &str) -> Option<i32> {
        fuzzy(q, hay).map(|(s, _)| s)
    }

    #[test]
    fn matches_are_subsequences() {
        assert!(score("os", "Open Settings").is_some()); // initials
        assert!(score("settings", "Open Settings").is_some());
        assert!(score("", "anything").is_some()); // empty query matches
        assert!(score("xyz", "Open Settings").is_none()); // not a subsequence
        assert!(score("ngs", "Open Settings").is_some()); // mid-word run
    }

    #[test]
    fn case_insensitive() {
        assert!(score("OPEN", "open settings").is_some());
        assert!(score("open", "OPEN SETTINGS").is_some());
    }

    #[test]
    fn word_boundary_beats_scatter() {
        // "os" as two word-initials should outscore the same letters mid-word.
        let initials = score("os", "Open Settings").unwrap();
        let scattered = score("os", "Booomerang Stew").unwrap();
        assert!(initials > scattered, "{initials} !> {scattered}");
    }

    #[test]
    fn prefix_and_consecutive_win() {
        // A contiguous prefix run outscores the same letters scattered mid-word.
        let prefix = score("set", "Settings").unwrap();
        let scattered = score("set", "Basement").unwrap(); // s..e....t, all mid-word
        assert!(prefix > scattered, "{prefix} !> {scattered}");

        // Word-initials are also a strong signal — on par with a prefix run.
        let initials = score("os", "Open Settings").unwrap();
        let midword = score("os", "Chaos Theory").unwrap(); // 'o','s' inside "Chaos"
        assert!(initials > midword, "{initials} !> {midword}");
    }

    #[test]
    fn highlight_ranges_point_at_matched_chars() {
        let (_, ranges) = fuzzy("os", "Open Settings").unwrap();
        // O at byte 0, S at byte 5.
        let matched: String = ranges.iter().map(|r| &"Open Settings"[r.clone()]).collect();
        assert_eq!(matched, "OS");
    }

    #[test]
    fn keyword_match_included_without_title_highlight() {
        let settings = commands().into_iter().find(|c| c.id == "settings").unwrap();
        // "prefs" only matches the keyword, not the title — included, no highlight.
        let (_, ranges) = score_command("prefs", &settings).unwrap();
        assert!(ranges.is_empty());
        // "set" matches the title — included, with highlight ranges.
        let (_, ranges) = score_command("set", &settings).unwrap();
        assert!(!ranges.is_empty());
    }
}
