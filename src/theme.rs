//! A macOS look: 13 px type, neutral greys, one accent. Root derives rem from the theme's font
//! size (gpui-component defaults to 16 px), so the base size scales controls as well as text.

use std::rc::Rc;

use gpui_kit::{
    App, SharedString, Window,
    component::{Theme, ThemeConfig, ThemeMode, ThemeRegistry},
};
use serde::{Deserialize, Serialize};

pub const FONT_SIZE: f32 = 13.0;

/// The one accent: primary buttons, focus rings, switches, links. Dark enough for white
/// text at AA contrast (macOS's #007AFF is only 4.0:1).
const ACCENT: [&str; 3] = ["#0064E1", "#0058C6", "#004CAB"]; // base, hover, active

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    pub const ALL: [Appearance; 3] = [Appearance::System, Appearance::Light, Appearance::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Appearance::System => "System",
            Appearance::Light => "Light",
            Appearance::Dark => "Dark",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|appearance| appearance.label().eq_ignore_ascii_case(name))
    }
}

struct Palette {
    window: &'static str,
    /// Grouped-content surface (`GroupBox`), popovers.
    surface: &'static str,
    text: &'static str,
    secondary_text: &'static str,
    separator: &'static str,
    /// Secondary buttons and hover fills: base, hover, active.
    control: [&'static str; 3],
    /// Input and switch-off borders/fills.
    control_border: &'static str,
    /// Track behind `TabBar::segmented`. Its selected segment is drawn in `window`, so the
    /// track must contrast with the window colour: lighter-than-window in dark mode reads as
    /// a hole, so dark uses a darker track.
    segmented_track: &'static str,
    /// Unselected segment labels, drawn on `segmented_track`.
    tab_text: &'static str,
    /// Links, drawn on `window` and `surface`.
    link: &'static str,
}

const LIGHT: Palette = Palette {
    window: "#F5F5F5",
    surface: "#FFFFFF",
    text: "#1D1D1F",
    secondary_text: "#6E6E73",
    separator: "#E2E2E4",
    control: ["#E8E8EA", "#DDDDE0", "#D2D2D5"],
    control_border: "#D1D1D6",
    segmented_track: "#E3E3E6",
    tab_text: "#5C5C61",
    link: "#0064E1",
};

const DARK: Palette = Palette {
    window: "#1E1E1E",
    surface: "#2A2A2A",
    text: "#E8E8EA",
    secondary_text: "#98989D",
    separator: "#3A3A3C",
    control: ["#3A3A3C", "#454547", "#505052"],
    control_border: "#4A4A4C",
    segmented_track: "#121212",
    tab_text: "#98989D",
    link: "#4C9EFF",
};

/// Call once, after `gpui_kit::init`.
pub fn install(cx: &mut App) {
    let registry = ThemeRegistry::global(cx);
    let mut light = (**registry.default_light_theme()).clone();
    let mut dark = (**registry.default_dark_theme()).clone();
    refine(&mut light, &LIGHT);
    refine(&mut dark, &DARK);
    let theme = Theme::global_mut(cx);
    theme.light_theme = Rc::new(light);
    theme.dark_theme = Rc::new(dark);
}

/// `System` reads the window's current OS appearance; call again when that changes.
pub fn apply(appearance: Appearance, window: &mut Window, cx: &mut App) {
    match appearance {
        Appearance::System => Theme::sync_system_appearance(Some(window), cx),
        Appearance::Light => Theme::change(ThemeMode::Light, Some(window), cx),
        Appearance::Dark => Theme::change(ThemeMode::Dark, Some(window), cx),
    }
}

fn refine(config: &mut ThemeConfig, p: &Palette) {
    config.font_size = Some(FONT_SIZE);
    let c = &mut config.colors;
    let set = |slot: &mut Option<SharedString>, hex: &str| *slot = Some(hex.to_string().into());

    set(&mut c.background, p.window);
    set(&mut c.foreground, p.text);
    set(&mut c.muted_foreground, p.secondary_text);
    set(&mut c.border, p.separator);
    set(&mut c.title_bar, p.window);
    set(&mut c.title_bar_border, p.separator);
    set(&mut c.group_box, p.surface);
    set(&mut c.group_box_foreground, p.text);
    set(&mut c.group_box_title_foreground, p.secondary_text);
    set(&mut c.popover, p.surface);
    set(&mut c.popover_foreground, p.text);

    set(&mut c.secondary, p.control[0]);
    set(&mut c.secondary_hover, p.control[1]);
    set(&mut c.secondary_active, p.control[2]);
    set(&mut c.secondary_foreground, p.text);
    set(&mut c.muted, p.control[0]);
    set(&mut c.accent, p.control[0]); // gpui-component's "accent" is a hover fill, not the brand
    set(&mut c.accent_foreground, p.text);
    set(&mut c.list_hover, p.control[0]);
    set(&mut c.input, p.control_border);

    set(&mut c.tab_bar_segmented, p.segmented_track);
    set(&mut c.tab_foreground, p.tab_text);
    set(&mut c.tab_active_foreground, p.text);
    set(&mut c.switch, p.control_border);
    set(&mut c.switch_thumb, "#FFFFFF");

    set(&mut c.primary, ACCENT[0]);
    set(&mut c.primary_hover, ACCENT[1]);
    set(&mut c.primary_active, ACCENT[2]);
    set(&mut c.primary_foreground, "#FFFFFF");
    set(&mut c.ring, ACCENT[0]);
    set(&mut c.link, p.link);
}

#[cfg(test)]
mod tests {
    use super::{ACCENT, DARK, LIGHT, Palette};

    /// WCAG 2 contrast ratio between two `#RRGGBB` colours.
    fn contrast(a: &str, b: &str) -> f64 {
        let luminance = |hex: &str| {
            let rgb = u32::from_str_radix(hex.trim_start_matches('#'), 16)
                .expect("palette colours are #RRGGBB");
            let channel = |shift: u32| {
                let c = f64::from((rgb >> shift) & 0xFF) / 255.0;
                if c <= 0.04045 {
                    c / 12.92
                } else {
                    ((c + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0)
        };
        let (la, lb) = (luminance(a), luminance(b));
        (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
    }

    fn assert_aa(text: &str, background: &str, what: &str) {
        let ratio = contrast(text, background);
        assert!(
            ratio >= 4.5,
            "{what}: {text} on {background} is {ratio:.2}:1"
        );
    }

    #[test]
    fn text_meets_aa_contrast_in_both_modes() {
        for (mode, p) in [("light", &LIGHT), ("dark", &DARK)] {
            let p: &Palette = p;
            let pairs = [
                ("text", p.text, p.window),
                ("text", p.text, p.surface),
                ("secondary text", p.secondary_text, p.window),
                ("secondary text", p.secondary_text, p.surface),
                ("unselected tab", p.tab_text, p.segmented_track),
                ("link", p.link, p.window),
                ("link", p.link, p.surface),
            ];
            for (what, text, background) in pairs {
                assert_aa(text, background, &format!("{mode} {what}"));
            }
        }
    }

    #[test]
    fn white_text_meets_aa_contrast_on_every_accent_state() {
        for hex in ACCENT {
            assert_aa("#FFFFFF", hex, "accent");
        }
    }
}
