mod debian_sysroot;

pub use self::debian_sysroot::DebianSysroot;
pub use self::debian_sysroot::DebianSysrootBuilder;
use std::process::Command;

/// On windows, npm is provided as this.
#[cfg(windows)]
const NPM_BIN: &str = "npm.cmd";

/// On other platforms, it is simply the command name.
#[cfg(not(windows))]
const NPM_BIN: &str = "npm";

/// Start building an npm command.
pub fn npm() -> Command {
    Command::new(NPM_BIN)
}
