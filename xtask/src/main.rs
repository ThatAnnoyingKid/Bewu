use anyhow::ensure;
use anyhow::Context;
use cargo_metadata::MetadataCommand;
use std::process::Command;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
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
    Run(RunOptions),
}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "build", description = "build the entire project")]
struct BuildOptions {}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "run", description = "run the entire project")]
struct RunOptions {}

fn build_frontend(metadata: &cargo_metadata::Metadata) -> anyhow::Result<()> {
    let frontend_dir = metadata.workspace_root.join("frontend");
    let output = Command::new(NPM_BIN)
        .current_dir(&frontend_dir)
        .args(["run", "build"])
        .status()
        .context("failed to run npm")?;
    ensure!(output.success(), "failed to run npm");

    let dist_dir = frontend_dir.join("dist");
    let public_dir = metadata.workspace_root.join("bewu/public");
    for entry in WalkDir::new(&dist_dir) {
        let entry = entry?;
        let entry_path = entry.path();
        let relative_path = entry_path.strip_prefix(&dist_dir)?;
        let file_type = entry.file_type();

        let dest_path = public_dir.join_os(relative_path);
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

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();

    match options.subcommand {
        Subcommand::Build(_options) => {
            let metadata = MetadataCommand::new().exec()?;

            build_frontend(&metadata)?;

            let output = Command::new("cargo")
                .current_dir(metadata.workspace_root.join("bewu"))
                .args(["build", "--bin", "bewu"])
                .status()
                .context("failed to spawn command")?;
            ensure!(output.success(), "failed to run cargo");
        }
        Subcommand::Run(_options) => {
            let running = Arc::new(AtomicBool::new(true));
            ctrlc::set_handler(move || {
                running.store(false, Ordering::SeqCst);
            })?;

            let metadata = MetadataCommand::new().exec()?;

            build_frontend(&metadata)?;

            let handle = std::thread::spawn(move || {
                let output = Command::new("cargo")
                    .current_dir(metadata.workspace_root.join("bewu"))
                    .args(["run", "--bin", "bewu"])
                    .stdout(Stdio::inherit())
                    .stdin(Stdio::null())
                    .stderr(Stdio::inherit())
                    .status()
                    .context("failed to spawn command")?;
                ensure!(output.success(), "failed to run cargo");

                Ok(())
            });

            handle.join().ok().context("server thread panicked")??;
        }
    }

    Ok(())
}
