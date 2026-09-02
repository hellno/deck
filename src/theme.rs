//! Theme — a refined dark/light palette with a selectable brand accent.
//!
//! gpui-component's stock dark theme is near pure-black-on-white, which reads
//! harsh. The common pattern (Linear, GitHub, Zed) is: a *soft* near-black with
//! slightly-elevated surfaces, muted secondary text, and a single saturated
//! **accent** that carries the brand. We build that by cloning the built-in
//! `ThemeConfig` and overriding ~20 color tokens, so it survives light/dark
//! toggles (gpui-component re-applies the config on every `Theme::change`).

use std::rc::Rc;

use gpui::{App, SharedString};
use gpui_component::{Theme, ThemeConfig, ThemeMode, ThemeRegistry};
use serde::{Deserialize, Serialize};

/// The brand accent. This is the knob that makes the app feel like *yours* — the
/// settings page lets the user pick one and it re-themes the whole app live.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Accent {
    #[default]
    Indigo,
    Blue,
    Violet,
    Emerald,
    Amber,
    Rose,
}

impl Accent {
    pub const ALL: [Accent; 6] = [
        Accent::Indigo,
        Accent::Blue,
        Accent::Violet,
        Accent::Emerald,
        Accent::Amber,
        Accent::Rose,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Accent::Indigo => "Indigo",
            Accent::Blue => "Blue",
            Accent::Violet => "Violet",
            Accent::Emerald => "Emerald",
            Accent::Amber => "Amber",
            Accent::Rose => "Rose",
        }
    }

    /// `(base, hover, active)` hex for the accent. `base` is also the swatch color.
    fn ramp(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Accent::Indigo => ("#4F46E5", "#4338CA", "#3730A3"),
            Accent::Blue => ("#2563EB", "#1D4ED8", "#1E40AF"),
            Accent::Violet => ("#7C3AED", "#6D28D9", "#5B21B6"),
            Accent::Emerald => ("#047857", "#065F46", "#064E3B"),
            Accent::Amber => ("#B45309", "#92400E", "#78350F"),
            Accent::Rose => ("#E11D48", "#BE123C", "#9F1239"),
        }
    }

    /// The swatch / mark color as an `0xRRGGBB` value, for `gpui::rgb(..)` in the UI.
    pub fn rgb(self) -> u32 {
        match self {
            Accent::Indigo => 0x4F46E5,
            Accent::Blue => 0x2563EB,
            Accent::Violet => 0x7C3AED,
            Accent::Emerald => 0x047857,
            Accent::Amber => 0xB45309,
            Accent::Rose => 0xE11D48,
        }
    }
}

/// Install (or re-install) the refined theme for `accent`, then apply `mode`.
/// Call once at startup and again whenever the user changes accent or mode.
pub fn install(cx: &mut App, accent: Accent, mode: ThemeMode) {
    // Ensure the Theme global exists (first call seeds it from the registry).
    Theme::change(mode, None, cx);

    let registry = ThemeRegistry::global(cx);
    let mut dark = (**registry.default_dark_theme()).clone();
    let mut light = (**registry.default_light_theme()).clone();
    refine(&mut dark, accent, true);
    refine(&mut light, accent, false);

    let theme = Theme::global_mut(cx);
    theme.dark_theme = Rc::new(dark);
    theme.light_theme = Rc::new(light);

    // Re-apply so the (possibly already-open) windows pick up the new config.
    Theme::change(mode, None, cx);
    cx.refresh_windows();
}

fn refine(config: &mut ThemeConfig, accent: Accent, dark: bool) {
    let (primary, primary_hover, primary_active) = accent.ramp();
    let c = &mut config.colors;
    let set = |slot: &mut Option<SharedString>, hex: &str| *slot = Some(hex.to_string().into());

    // Brand accent (shared across modes). `primary` is the brand color; the
    // `accent` token is a *subtle surface* (ghost-button / menu hover), not the brand.
    set(&mut c.primary, primary);
    set(&mut c.primary_hover, primary_hover);
    set(&mut c.primary_active, primary_active);
    set(&mut c.primary_foreground, "#FFFFFF");
    set(&mut c.ring, primary);

    if dark {
        set(&mut c.background, "#0C0D11"); // soft near-black, faint blue cast
        set(&mut c.foreground, "#E6E7EB"); // soft white, not #FFF
        set(&mut c.secondary, "#16171D"); // cards / surfaces
        set(&mut c.secondary_foreground, "#E6E7EB");
        set(&mut c.muted, "#1B1C23");
        set(&mut c.muted_foreground, "#8A8C99"); // secondary text
        set(&mut c.border, "#262833");
        set(&mut c.input, "#1B1C23");
        set(&mut c.popover, "#16171D");
        set(&mut c.popover_foreground, "#E6E7EB");
        set(&mut c.title_bar, "#0C0D11");
        set(&mut c.title_bar_border, "#1B1C23");
        set(&mut c.sidebar, "#0F1014");
        set(&mut c.accent, "#1F2029"); // subtle hover surface
        set(&mut c.accent_foreground, "#E6E7EB");
    } else {
        set(&mut c.background, "#FBFBFC");
        set(&mut c.foreground, "#16171D");
        set(&mut c.secondary, "#F1F2F4");
        set(&mut c.secondary_foreground, "#16171D");
        set(&mut c.muted, "#F1F2F4");
        set(&mut c.muted_foreground, "#6B6D78");
        set(&mut c.border, "#E4E5EA");
        set(&mut c.input, "#FFFFFF");
        set(&mut c.popover, "#FFFFFF");
        set(&mut c.popover_foreground, "#16171D");
        set(&mut c.title_bar, "#FBFBFC");
        set(&mut c.title_bar_border, "#E4E5EA");
        set(&mut c.sidebar, "#F6F7F9");
        set(&mut c.accent, "#F1F2F4");
        set(&mut c.accent_foreground, "#16171D");
    }
}

#[cfg(test)]
mod tests {
    use gpui_component::ThemeConfig;

    use super::{refine, Accent};

    fn contrast_with_white(hex: &str) -> f64 {
        let value = u32::from_str_radix(hex.trim_start_matches('#'), 16)
            .expect("theme ramps contain valid six-digit hex colors");
        let channel = |shift| {
            let value = f64::from(((value >> shift) & 0xff_u32) as u8) / 255.0;
            if value <= 0.04045 {
                value / 12.92
            } else {
                ((value + 0.055) / 1.055).powf(2.4)
            }
        };
        let luminance = 0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0);
        1.05 / (luminance + 0.05)
    }

    #[test]
    fn every_primary_state_meets_aa_contrast_with_white_text() {
        for accent in Accent::ALL {
            for hex in [accent.ramp().0, accent.ramp().1, accent.ramp().2] {
                let contrast = contrast_with_white(hex);
                assert!(
                    contrast >= 4.5,
                    "{} {hex} has only {contrast:.2}:1 contrast",
                    accent.label()
                );
            }
        }
    }

    #[test]
    fn swatch_rgb_matches_the_primary_ramp_color() {
        for accent in Accent::ALL {
            let ramp_rgb = u32::from_str_radix(accent.ramp().0.trim_start_matches('#'), 16)
                .expect("theme ramps contain valid six-digit hex colors");
            assert_eq!(accent.rgb(), ramp_rgb, "{} swatch drifted", accent.label());
        }
    }

    #[test]
    fn refine_applies_accessible_brand_tokens_in_both_modes() {
        for dark in [false, true] {
            for accent in Accent::ALL {
                let mut config = ThemeConfig::default();
                refine(&mut config, accent, dark);

                assert_eq!(config.colors.primary.as_deref(), Some(accent.ramp().0));
                assert_eq!(
                    config.colors.primary_hover.as_deref(),
                    Some(accent.ramp().1)
                );
                assert_eq!(
                    config.colors.primary_active.as_deref(),
                    Some(accent.ramp().2)
                );
                assert_eq!(config.colors.primary_foreground.as_deref(), Some("#FFFFFF"));
                assert_eq!(config.colors.ring.as_deref(), Some(accent.ramp().0));
                assert!(config.colors.background.is_some());
                assert!(config.colors.foreground.is_some());
            }
        }
    }
}
