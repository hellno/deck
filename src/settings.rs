//! Settings — typed preferences persisted to the platform config directory.
//!
//! This is the **mainstream, dependency-light Rust pattern**: a `serde` struct
//! written as JSON into the OS config dir (`directories` finds it — on macOS
//! that's `~/Library/Application Support/<id>/settings.json`). No database, no
//! framework. See `docs/LEARNINGS.md` for how this compares to `confy` and to
//! Zed's layered settings system.

use std::path::PathBuf;

use directories::ProjectDirs;
use gpui_component::ThemeMode;
use serde::{Deserialize, Serialize};

use crate::theme::Accent;

// Reverse-DNS used for the config dir. Keep in sync with the bundle identifier
// in Cargo.toml when you fork. (qualifier, organization, application)
const QUALIFIER: &str = "{{bundle_qualifier}}";
const ORGANIZATION: &str = "{{bundle_org}}";
const APPLICATION: &str = "{{project-name}}";

/// Persisted theme preference. We keep our own enum (rather than reusing
/// gpui-component's `ThemeMode`) so the on-disk format is ours to control.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThemeModePref {
    #[default]
    Dark,
    Light,
}

impl ThemeModePref {
    pub fn to_gpui(self) -> ThemeMode {
        match self {
            ThemeModePref::Dark => ThemeMode::Dark,
            ThemeModePref::Light => ThemeMode::Light,
        }
    }
}

/// Everything the app remembers between launches. Add fields freely — the
/// `#[serde(default)]` makes older config files forward-compatible (and lets serde
/// silently ignore stale keys like a removed `overlay_anchor`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_mode: ThemeModePref,
    pub accent: Accent,
    pub display_name: String,
    pub launch_minimized: bool,
    /// Whether to open the floating overlay surface on launch. Defaults `true` so that
    /// compiling `--features overlay` shows it immediately (mirrors how `--features tray`
    /// just works); the feature flag is the only *build* gate. Override per-run without
    /// editing this file via `DECK_OVERLAY=0|1` (see `overlay::install`).
    pub overlay_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_mode: ThemeModePref::Dark,
            accent: Accent::default(),
            display_name: String::new(),
            launch_minimized: false,
            overlay_enabled: true,
        }
    }
}

impl Settings {
    fn path() -> Option<PathBuf> {
        ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
            .map(|dirs| dirs.config_dir().join("settings.json"))
    }

    /// Human-readable path, shown in the settings UI so users know where prefs live.
    pub fn config_path_display() -> String {
        Self::path()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<unavailable>".to_string())
    }

    /// Load from disk, falling back to defaults on a missing/corrupt file.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Write to disk, creating the config dir if needed. Returns the IO/serialize
    /// error so a caller can actually handle it; UI call sites use `save_best_effort`.
    ///
    /// Keep this OFF the UI hot path: it rewrites the whole file, cheap only because
    /// this struct is tiny. Persist at a coarse boundary (blur/commit) or on the
    /// background executor — never on a per-keystroke `InputEvent::Change`. The "why"
    /// and the debounce option are in `docs/LEARNINGS.md` §17.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(&path, json)
    }

    /// Best-effort persist for UI call sites: a lost preference write should never
    /// crash or block the UI, so we log and move on. Prefer `save` (and real error
    /// handling) when the write is load-bearing — e.g. an agent fork's chat history.
    pub fn save_best_effort(&self) {
        if let Err(err) = self.save() {
            eprintln!("{{project-name}}: could not save settings: {err}");
        }
    }
}
