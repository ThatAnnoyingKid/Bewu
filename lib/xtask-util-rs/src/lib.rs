use std::process::Command;

/// On windows, npm is provided as this.
#[cfg(windows)]
const NPM_BIN: &str = "npm.cmd";

/// On other platforms, it is simply the command name.
#[cfg(not(windows))]
const NPM_BIN: &str = "npm";

/// Start buulding an npm command.
pub fn npm() -> Command {
    Command::new(NPM_BIN)
}
