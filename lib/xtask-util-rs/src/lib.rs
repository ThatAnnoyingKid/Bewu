use std::process::Command;

/// On windows, pnpm is provided as "npm.cmd".
/// On other platforms, it is simply "npm".
const NPM_BIN: &str = if cfg!(windows) { "npm.cmd" } else { "npm" };

/// Start building an npm command.
pub fn npm() -> Command {
    Command::new(NPM_BIN)
}

/// On windows, pnpm is provided as "pnpm.cmd".
/// On other platforms, it is simply "pnpm".
const PNPM_BIN: &str = if cfg!(windows) { "pnpm.cmd" } else { "pnpm" };

/// Start building a pnpm command.
pub fn pnpm() -> Command {
    Command::new(PNPM_BIN)
}

/// Get the debian arch for a given triple
pub fn get_debian_arch(triple: &str) -> Option<&'static str> {
    match triple {
        "x86_64-unknown-linux-gnu" => Some("amd64"),
        "aarch64-unknown-linux-gnu" => Some("arm64"),
        "armv7-unknown-linux-gnueabihf" => Some("armhf"),
        _ => None,
    }
}
