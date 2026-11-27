use std::process::ExitCode;

const USAGE: &str = "usage: {{project-name}} [--help] [--version]

environment (see src/launch.rs):
  APP_DRY_RUN=1          separate settings file, no activation, nothing irreversible
  APP_PAGE=<page>        start on a page: home | settings
  APP_APPEARANCE=<mode>  system | light | dark, for this run only
  APP_FIXTURES=1         seed sample data";

fn main() -> ExitCode {
    let (mut help, mut version) = (false, false);
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => help = true,
            "-V" | "--version" => version = true,
            // Launch Services may append a process serial number (`-psn_0_12345`).
            psn if cfg!(target_os = "macos") && psn.starts_with("-psn_") => {}
            other => {
                let bin = env!("CARGO_BIN_NAME");
                eprintln!("{bin}: unknown argument {other:?}\n\n{USAGE}");
                return ExitCode::from(2);
            }
        }
    }
    if help {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    if version {
        println!("{} {}", env!("CARGO_BIN_NAME"), env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    match {{crate_name}}::Launch::from_env() {
        Ok(launch) => {
            {{crate_name}}::run(launch);
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{}: {err}", env!("CARGO_BIN_NAME"));
            ExitCode::from(2)
        }
    }
}
