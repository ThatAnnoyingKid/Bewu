use anyhow::ensure;
use anyhow::Context;
use cargo_metadata::MetadataCommand;
use std::process::Command;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use walkdir::WalkDir;

const SERVER_BIN: &str = "bewu";

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
    Fmt(FmtOptions),
    BuildDeb(BuildDebOptions),
    DeployDeb(DeployDebOptions),
}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "build", description = "build the entire project")]
struct BuildOptions {}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "run", description = "run the entire project")]
struct RunOptions {}

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "fmt", description = "fmt the entire project")]
struct FmtOptions {}

#[derive(Debug, argh::FromArgs)]
#[argh(
    subcommand,
    name = "build-deb",
    description = "build a deb for this project"
)]
struct BuildDebOptions {
    #[argh(option, description = "the target triple to build")]
    target: String,
}

#[derive(Debug, argh::FromArgs)]
#[argh(
    subcommand,
    name = "deploy-deb",
    description = "deploy a deb for this project"
)]
struct DeployDebOptions {
    #[argh(option, description = "the target triple to deploy")]
    target: String,

    #[argh(option, description = "the server to deploy to")]
    hostname: String,
}

fn build_frontend(metadata: &cargo_metadata::Metadata) -> anyhow::Result<()> {
    let frontend_dir = metadata.workspace_root.join("frontend");
    let output = xtask_util::npm()
        .current_dir(&frontend_dir)
        .args(["run", "build"])
        .status()
        .context("failed to run npm")?;
    ensure!(output.success(), "failed to run npm");

    let dist_dir = frontend_dir.join("dist");
    let public_dir = metadata.workspace_root.join("server/public");
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

fn fmt_all(metadata: &cargo_metadata::Metadata) -> anyhow::Result<()> {
    let output = xtask_util::npm()
        .current_dir(&metadata.workspace_root.join("frontend"))
        .args(["run", "fmt"])
        .status()
        .context("failed to spawn command")?;
    ensure!(output.success(), "failed to run cargo");

    let output = Command::new("cargo")
        .current_dir(&metadata.workspace_root)
        .args(["fmt", "--all"])
        .status()
        .context("failed to spawn command")?;
    ensure!(output.success(), "failed to run cargo");

    Ok(())
}

fn build_deb(metadata: &cargo_metadata::Metadata, target: &str) -> anyhow::Result<()> {
    build_frontend(metadata)?;
    fmt_all(metadata)?;

    let mut command = Command::new("debian-sysroot-build");
    command
        .current_dir(metadata.workspace_root.join("server"))
        .args(["--target", target])
        .args(["--package", SERVER_BIN])
        .args([
            "--install-package",
            "libc6",
            "--install-package",
            "libc6-dev",
            "--install-package",
            "linux-libc-dev",
            "--install-package",
            "libgcc-12-dev",
        ]);
    let output = command.status().context("failed to spawn command")?;
    ensure!(output.success(), "failed to run debian-sysroot-build");

    let output = Command::new("cargo")
        .current_dir(metadata.workspace_root.join("server"))
        .args(["deb", "--target", target, "--no-build", "--no-strip"])
        .status()
        .context("failed to spawn command")?;
    ensure!(output.success(), "failed to run cargo deb");

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();

    match options.subcommand {
        Subcommand::Build(_options) => {
            let metadata = MetadataCommand::new().exec()?;

            build_frontend(&metadata)?;

            let output = Command::new("cargo")
                .current_dir(metadata.workspace_root.join("server"))
                .args(["build", "--bin", SERVER_BIN])
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
                    .current_dir(metadata.workspace_root.join("server"))
                    .args(["run", "--bin", SERVER_BIN])
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
        Subcommand::Fmt(_options) => {
            let metadata = MetadataCommand::new().exec()?;

            fmt_all(&metadata)?;
        }
        Subcommand::BuildDeb(options) => {
            let metadata = MetadataCommand::new().exec()?;

            build_deb(&metadata, options.target.as_str())?;
        }
        Subcommand::DeployDeb(options) => {
            let metadata = MetadataCommand::new().exec()?;

            let server_package_metadata = metadata
                .packages
                .iter()
                .find(|package| package.name == SERVER_BIN)
                .with_context(|| format!("missing package \"{SERVER_BIN}\""))?;

            let hostname = options.hostname.as_str();
            let target = options.target.as_str();

            build_deb(&metadata, options.target.as_str())?;

            let debian_arch = xtask_util::get_debian_arch(target)
                .with_context(|| format!("failed to get debian arch for \"{target}\""))?;

            let deb_version = &server_package_metadata.version;
            let deb_revision = server_package_metadata
                .metadata
                .as_object()
                .and_then(|metadata| metadata.get("deb")?.as_object()?.get("revision")?.as_str())
                .unwrap_or("1");
            let deb_name = format!("{SERVER_BIN}_{deb_version}-{deb_revision}_{debian_arch}.deb");
            let output = Command::new("deploy-deb")
                .current_dir(&metadata.workspace_root)
                .args([
                    format!("target/{target}/debian/{deb_name}").as_str(),
                    hostname,
                ])
                .status()
                .context("failed to spawn command")?;
            ensure!(output.success(), "failed to run deploy-deb");
        }
    }

    Ok(())
}
