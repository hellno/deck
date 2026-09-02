//! Settings — typed preferences persisted to the platform config directory.
//!
//! This is the **mainstream, dependency-light Rust pattern**: a `serde` struct
//! written as JSON into the OS config dir (`directories` finds it — on macOS
//! that's `~/Library/Application Support/<id>/settings.json`). No database, no
//! framework. See `docs/LEARNINGS.md` for how this compares to `confy` and to
//! Zed's layered settings system.

use std::path::{Path, PathBuf};

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
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub theme_mode: ThemeModePref,
    pub accent: Accent,
    pub display_name: String,
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
        Self::load_from(&path)
    }

    fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Write to disk atomically, creating the config dir if needed. Returns the
    /// IO/serialize error so a caller can actually handle it; UI call sites use
    /// `save_best_effort`.
    ///
    /// Keep this OFF the UI hot path: it rewrites the whole file, cheap only because
    /// this struct is tiny. Persist at a coarse boundary (blur/commit) or on the
    /// background executor — never on a per-keystroke `InputEvent::Change`. The "why"
    /// and the debounce option are in `docs/LEARNINGS.md` §17.
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = Self::path() else {
            return Ok(());
        };
        self.save_to(&path)
    }

    fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let temporary_path = path.with_extension("json.tmp");
        std::fs::write(&temporary_path, json)?;
        let rename_result = std::fs::rename(&temporary_path, path);
        if rename_result.is_err() {
            // Preserve the original rename error; cleanup is only best effort.
            drop(std::fs::remove_file(&temporary_path));
        }
        rename_result
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

#[cfg(test)]
mod tests {
    use std::{
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{Settings, ThemeModePref};
    use crate::theme::Accent;

    fn test_dir(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "deck-settings-{name}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn save_to_round_trips_atomically() {
        let root = test_dir("round-trip");
        let path = root.join("nested/settings.json");
        let settings = Settings {
            theme_mode: ThemeModePref::Light,
            accent: Accent::Rose,
            display_name: "Ada".to_string(),
            overlay_enabled: false,
        };

        settings.save_to(&path).expect("settings save succeeds");

        assert_eq!(Settings::load_from(&path), settings);
        assert!(!path.with_extension("json.tmp").exists());
        std::fs::remove_dir_all(root).expect("test directory is removable");
    }

    #[test]
    fn load_from_missing_or_corrupt_file_uses_defaults() {
        let root = test_dir("fallback");
        let path = root.join("settings.json");

        assert_eq!(Settings::load_from(&path), Settings::default());
        std::fs::create_dir_all(&root).expect("test directory is created");
        std::fs::write(&path, "not json").expect("corrupt fixture is written");
        assert_eq!(Settings::load_from(&path), Settings::default());

        std::fs::remove_dir_all(root).expect("test directory is removable");
    }

    #[test]
    fn load_from_ignores_removed_keys() {
        let root = test_dir("legacy");
        let path = root.join("settings.json");
        std::fs::create_dir_all(&root).expect("test directory is created");
        std::fs::write(&path, r#"{"launch_minimized":true,"display_name":"Grace"}"#)
            .expect("legacy fixture is written");

        let settings = Settings::load_from(&path);

        assert_eq!(settings.display_name, "Grace");
        assert_eq!(settings.theme_mode, ThemeModePref::Dark);
        assert!(settings.overlay_enabled);
        std::fs::remove_dir_all(root).expect("test directory is removable");
    }

    #[test]
    fn save_to_cleans_up_after_rename_failure() {
        let root = test_dir("rename-failure");
        let path = root.join("settings.json");
        std::fs::create_dir_all(&path).expect("destination directory is created");

        let error = Settings::default()
            .save_to(&path)
            .expect_err("renaming a file over a directory must fail");

        assert_ne!(error.kind(), std::io::ErrorKind::NotFound);
        assert!(!path.with_extension("json.tmp").exists());
        std::fs::remove_dir_all(root).expect("test directory is removable");
    }

    #[test]
    fn save_to_propagates_parent_creation_errors() {
        let root = test_dir("parent-failure");
        std::fs::create_dir_all(&root).expect("test directory is created");
        let parent_file = root.join("not-a-directory");
        std::fs::write(&parent_file, "occupied").expect("blocking file is written");

        Settings::default()
            .save_to(&parent_file.join("settings.json"))
            .expect_err("a file cannot be used as a parent directory");

        std::fs::remove_dir_all(root).expect("test directory is removable");
    }
}
