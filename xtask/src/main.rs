use anyhow::ensure;
use anyhow::Context;
use std::path::Path;
use std::process::Command;
use walkdir::WalkDir;

#[cfg(windows)]
const NPM_BIN: &str = "npm.cmd";

#[cfg(not(windows))]
const NPM_BIN: &str = "npm";

#[derive(Debug, argh::FromArgs)]
#[argh(description = "a build tool")]
struct Options {
    #[argh(subcommand)]
    subcommand: Subcommand,
}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand)]
enum Subcommand {
    Build(BuildOptions),
}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "build", description = "build the entire project")]
struct BuildOptions {}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();

    match options.subcommand {
        Subcommand::Build(_options) => {
            let current_dir = std::env::current_dir().context("failed to get current dir")?;
            let frontend_dir = current_dir.join("../frontend");

            let output = Command::new(NPM_BIN)
                .current_dir(&frontend_dir)
                .args(["run", "build"])
                .status()
                .context("failed to run npm")?;
            ensure!(output.success(), "failed to run npm");

            let output = Command::new("cargo")
                .args(["build", "--bin", "bewu"])
                .status()
                .context("failed to spawn command")?;
            ensure!(output.success(), "failed to run cargo");

            let dist_dir = frontend_dir.join("dist");
            for entry in WalkDir::new(&dist_dir) {
                let entry = entry?;
                let entry_path = entry.path();
                let relative_path = entry_path.strip_prefix(&dist_dir)?;
                let file_type = entry.file_type();

                let dest_path = Path::new("../bewu/public").join(relative_path);
                if file_type.is_file() {
                    std::fs::copy(entry_path, dest_path)?;
                } else {
                    match std::fs::create_dir(dest_path) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                        Err(e) => {
                            Err(e)?;
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
