//! Launch options from the `APP_*` environment variables listed in `main.rs`.

use gpui_kit::Global;

use crate::theme::Appearance;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Home,
    Settings,
}

impl Page {
    pub const ALL: [Page; 2] = [Page::Home, Page::Settings];

    pub fn label(self) -> &'static str {
        match self {
            Page::Home => "Home",
            Page::Settings => "Settings",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|page| page.label().eq_ignore_ascii_case(name))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Launch {
    pub dry_run: bool,
    pub page: Page,
    pub appearance: Option<Appearance>,
    pub fixtures: bool,
}

impl Global for Launch {}

impl Launch {
    /// Read the `APP_*` variables. An unrecognized value is an error, not a silent default.
    pub fn from_env() -> Result<Self, String> {
        // Non-UTF-8 values become U+FFFD and then fail to parse, rather than being ignored.
        Self::from_vars(|key| std::env::var_os(key).map(|v| v.to_string_lossy().into_owned()))
    }

    fn from_vars(var: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let flag = |key: &str| match var(key) {
            None => Ok(false),
            Some(value) => match value.to_ascii_lowercase().as_str() {
                "" | "0" | "false" => Ok(false),
                "1" | "true" => Ok(true),
                _ => Err(format!("{key}={value:?}: expected 1 or 0")),
            },
        };
        let page = match var("APP_PAGE") {
            None => Page::default(),
            Some(value) => Page::parse(&value)
                .ok_or_else(|| format!("APP_PAGE={value:?}: expected home or settings"))?,
        };
        let appearance = match var("APP_APPEARANCE") {
            None => None,
            Some(value) => Some(Appearance::parse(&value).ok_or_else(|| {
                format!("APP_APPEARANCE={value:?}: expected system, light or dark")
            })?),
        };
        Ok(Self {
            dry_run: flag("APP_DRY_RUN")?,
            page,
            appearance,
            fixtures: flag("APP_FIXTURES")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Launch, Page};
    use crate::theme::Appearance;

    fn launch(vars: &[(&str, &str)]) -> Result<Launch, String> {
        Launch::from_vars(|key| {
            vars.iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value.to_string())
        })
    }

    #[test]
    fn no_variables_means_a_normal_launch() {
        assert_eq!(launch(&[]), Ok(Launch::default()));
    }

    #[test]
    fn hooks_set_page_appearance_and_flags() {
        let launch = launch(&[
            ("APP_DRY_RUN", "1"),
            ("APP_PAGE", "settings"),
            ("APP_APPEARANCE", "dark"),
            ("APP_FIXTURES", "True"),
        ])
        .expect("valid hooks parse");
        assert!(launch.dry_run && launch.fixtures);
        assert_eq!(launch.page, Page::Settings);
        assert_eq!(launch.appearance, Some(Appearance::Dark));
    }

    #[test]
    fn unknown_values_are_errors() {
        // A mistyped dry-run flag must not silently fall back to the real settings file.
        assert!(launch(&[("APP_DRY_RUN", "yes")]).is_err());
        assert!(launch(&[("APP_PAGE", "inbox")]).is_err());
        assert!(launch(&[("APP_APPEARANCE", "sepia")]).is_err());
    }
}
