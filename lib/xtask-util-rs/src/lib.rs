use std::process::Command;

#[cfg(windows)]
const NPM_BIN: &str = "npm.cmd";

#[cfg(not(windows))]
const NPM_BIN: &str = "npm";

/// Start buulding an npm command.
pub fn npm() -> Command {
    Command::new(NPM_BIN)
}
