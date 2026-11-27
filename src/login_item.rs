//! Open at login (macOS): a per-user LaunchAgent, `~/Library/LaunchAgents/<bundle id>.plist`,
//! that starts the installed app's executable.

use std::{
    io,
    path::{Path, PathBuf},
};

use crate::settings::{APPLICATION, ORGANIZATION, QUALIFIER};

pub struct LoginItem {
    executable: Option<PathBuf>,
    enabled: bool,
}

impl LoginItem {
    /// Available only from an app installed in an Applications folder (a login item pointing
    /// into `target/` breaks on the next build or clean), and never in a dry run.
    pub fn detect(dry_run: bool) -> Self {
        let installed = |exe: &Path| {
            let home_apps =
                directories::BaseDirs::new().map(|dirs| dirs.home_dir().join("Applications"));
            (exe.starts_with("/Applications")
                || home_apps.is_some_and(|apps| exe.starts_with(apps)))
                && exe.to_string_lossy().contains(".app/Contents/MacOS/")
        };
        let executable = std::env::current_exe()
            .ok()
            .filter(|exe| !dry_run && installed(exe));
        // Unavailable reads as off, so the result doesn't depend on this machine's state.
        let enabled = executable.is_some() && agent_path().is_some_and(|path| path.exists());
        Self {
            executable,
            enabled,
        }
    }

    pub fn available(&self) -> bool {
        self.executable.is_some()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) -> io::Result<()> {
        let executable = self
            .executable
            .as_deref()
            .ok_or_else(|| io::Error::other("only available in the installed app"))?;
        let path = agent_path().ok_or_else(|| io::Error::other("no home directory"))?;
        if enabled {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&path, plist(&label(), executable))?;
        } else if let Err(err) = std::fs::remove_file(&path)
            && err.kind() != io::ErrorKind::NotFound
        {
            return Err(err);
        }
        self.enabled = enabled;
        Ok(())
    }
}

fn label() -> String {
    format!("{QUALIFIER}.{ORGANIZATION}.{APPLICATION}")
}

fn agent_path() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|dirs| {
        dirs.home_dir()
            .join("Library/LaunchAgents")
            .join(format!("{}.plist", label()))
    })
}

fn plist(label: &str, executable: &Path) -> String {
    let escape = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
</dict>
</plist>
"#,
        escape(label),
        escape(&executable.to_string_lossy())
    )
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    #[test]
    fn plist_runs_the_executable_at_load_and_escapes_paths() {
        let plist = super::plist(
            "com.example.app",
            Path::new("/Applications/A & B.app/Contents/MacOS/a"),
        );
        assert!(plist.contains("<string>com.example.app</string>"));
        assert!(plist.contains("<string>/Applications/A &amp; B.app/Contents/MacOS/a</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>\n    <true/>"));
    }

    #[test]
    fn unavailable_outside_a_bundle_and_in_dry_runs() {
        assert!(!super::LoginItem::detect(false).available());
        assert!(!super::LoginItem::detect(true).available());
    }
}
