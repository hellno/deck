//! Preferences as JSON in the platform config directory (macOS: `~/Library/Application
//! Support/<bundle id>/`). A dry run uses `settings.dry-run.json` instead of `settings.json`.

use std::{
    io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

use crate::theme::Appearance;

// The bundle identifier's parts, as in Cargo.toml's [package.metadata.bundle].
pub(crate) const QUALIFIER: &str = "{{bundle_qualifier}}";
pub(crate) const ORGANIZATION: &str = "{{bundle_org}}";
pub(crate) const APPLICATION: &str = "{{project-name}}";

/// With `#[serde(default)]`, missing keys take their default and unknown keys are ignored,
/// so files from older and newer versions still load.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub appearance: Appearance,
}

pub struct Loaded {
    pub settings: Settings,
    /// Why the file couldn't be read, for the UI.
    pub problem: Option<String>,
}

/// Where settings live. `None` never touches the disk (headless shots, tests).
pub struct SettingsFile {
    path: Option<PathBuf>,
}

impl SettingsFile {
    pub fn for_app(dry_run: bool) -> Self {
        let name = if dry_run {
            "settings.dry-run.json"
        } else {
            "settings.json"
        };
        Self {
            path: ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION)
                .map(|dirs| dirs.config_dir().join(name)),
        }
    }

    pub fn none() -> Self {
        Self { path: None }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Read the settings. A missing file means defaults. An unreadable one is moved aside
    /// first; if even that fails, saving is switched off for this session.
    pub fn load(&mut self) -> Loaded {
        let Some(path) = self.path.clone() else {
            return Loaded::defaults(None);
        };
        let error = match std::fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str(&text) {
                Ok(settings) => {
                    return Loaded {
                        settings,
                        problem: None,
                    };
                }
                Err(err) => err.to_string(),
            },
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Loaded::defaults(None),
            Err(err) => err.to_string(),
        };
        let aside = unreadable_path(&path);
        let problem = match std::fs::rename(&path, &aside) {
            Ok(()) => format!(
                "Your settings couldn't be read ({error}). The file was moved to {} and defaults are in use.",
                aside.display()
            ),
            Err(rename_err) => {
                self.path = None;
                format!(
                    "Your settings couldn't be read ({error}) or moved aside ({rename_err}). Changes won't be saved until you fix {}.",
                    path.display()
                )
            }
        };
        eprintln!("{}: {problem}", crate::APP_NAME);
        Loaded::defaults(Some(problem))
    }

    /// Writes atomically (temp file, then rename). Call it at a coarse boundary such as a
    /// click or blur, never per keystroke.
    pub fn save(&self, settings: &Settings) -> io::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
        // Per-process temp name: two running copies must not write into the same file.
        let temporary = temporary_path(path);
        std::fs::write(&temporary, json)?;
        let renamed = std::fs::rename(&temporary, path);
        if renamed.is_err() {
            drop(std::fs::remove_file(&temporary));
        }
        renamed
    }

    /// Logs a failed write and keeps the in-memory value.
    pub fn save_best_effort(&self, settings: &Settings) {
        if let Err(err) = self.save(settings) {
            eprintln!("{}: could not save settings: {err}", crate::APP_NAME);
        }
    }
}

impl Loaded {
    fn defaults(problem: Option<String>) -> Self {
        Self {
            settings: Settings::default(),
            problem,
        }
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    path.with_extension(format!("json.tmp-{}", std::process::id()))
}

/// `settings.json.unreadable-<unix time>`, with a counter if that name is taken, so an
/// earlier backup is never replaced.
fn unreadable_path(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs());
    let name = path.file_name().map_or_else(
        || "settings.json".into(),
        |name| name.to_string_lossy().into_owned(),
    );
    (0..)
        .map(|n| match n {
            0 => path.with_file_name(format!("{name}.unreadable-{stamp}")),
            n => path.with_file_name(format!("{name}.unreadable-{stamp}-{n}")),
        })
        .find(|candidate| !candidate.exists())
        .expect("an unused backup name exists")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{Settings, SettingsFile, temporary_path};
    use crate::theme::Appearance;

    fn test_dir(name: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("settings-{name}-{}-{nonce}", std::process::id()))
    }

    fn file_at(path: &std::path::Path) -> SettingsFile {
        SettingsFile {
            path: Some(path.to_path_buf()),
        }
    }

    #[test]
    fn save_round_trips_atomically() {
        let root = test_dir("round-trip");
        let path = root.join("nested/settings.json");
        let settings = Settings {
            appearance: Appearance::Dark,
        };

        file_at(&path).save(&settings).expect("save succeeds");

        let loaded = file_at(&path).load();
        assert_eq!(loaded.settings, settings);
        assert!(loaded.problem.is_none());
        assert!(!temporary_path(&path).exists());
        std::fs::remove_dir_all(root).expect("test directory is removable");
    }

    #[test]
    fn missing_file_loads_defaults_quietly() {
        let loaded = file_at(&test_dir("missing").join("settings.json")).load();
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.problem.is_none());
    }

    #[test]
    fn unreadable_files_are_moved_aside_never_overwritten() {
        let root = test_dir("corrupt");
        let path = root.join("settings.json");
        std::fs::create_dir_all(&root).expect("test directory is created");

        // Two recoveries within the same second must keep both backups.
        for content in ["first", "second"] {
            std::fs::write(&path, content).expect("corrupt fixture is written");
            let mut file = file_at(&path);
            let loaded = file.load();
            assert_eq!(loaded.settings, Settings::default());
            assert!(loaded.problem.is_some());
            file.save(&loaded.settings).expect("save succeeds");
        }

        let mut aside: Vec<String> = std::fs::read_dir(&root)
            .expect("test directory is readable")
            .map(|entry| entry.expect("entry is readable").path())
            .filter(|p| p.to_string_lossy().contains("settings.json.unreadable-"))
            .map(|p| std::fs::read_to_string(p).expect("moved file is readable"))
            .collect();
        aside.sort();
        assert_eq!(aside, ["first", "second"]);
        std::fs::remove_dir_all(root).expect("test directory is removable");
    }

    #[test]
    fn unknown_keys_are_ignored() {
        let root = test_dir("legacy");
        let path = root.join("settings.json");
        std::fs::create_dir_all(&root).expect("test directory is created");
        std::fs::write(&path, r#"{"appearance":"Light","display_name":"Grace"}"#)
            .expect("legacy fixture is written");

        let loaded = file_at(&path).load();
        assert_eq!(loaded.settings.appearance, Appearance::Light);
        assert!(loaded.problem.is_none());
        std::fs::remove_dir_all(root).expect("test directory is removable");
    }

    #[test]
    fn save_cleans_up_after_rename_failure() {
        let root = test_dir("rename-failure");
        let path = root.join("settings.json");
        std::fs::create_dir_all(&path).expect("destination directory is created");

        file_at(&path)
            .save(&Settings::default())
            .expect_err("renaming a file over a directory must fail");

        assert!(!temporary_path(&path).exists());
        std::fs::remove_dir_all(root).expect("test directory is removable");
    }
}
