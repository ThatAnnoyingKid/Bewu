mod debian_sysroot;
use anyhow::ensure;
use anyhow::Context;
use cargo_metadata::MetadataCommand;
pub use debian_sysroot::DebianSysrootBuilder;
use std::process::Command;

/// Get the debian arch for a given triple
pub fn get_debian_arch(triple: &str) -> Option<&'static str> {
    match triple {
        "x86_64-unknown-linux-gnu" => Some("amd64"),
        "aarch64-unknown-linux-gnu" => Some("arm64"),
        "armv7-unknown-linux-gnueabihf" => Some("armhf"),
        _ => None,
    }
}

/// Get the gcc triple for a given Rust triple
pub fn get_gcc_triple(triple: &str) -> Option<&'static str> {
    match triple {
        "x86_64-unknown-linux-gnu" => Some("x86_64-linux-gnu"),
        "aarch64-unknown-linux-gnu" => Some("aarch64-linux-gnu"),
        _ => None,
    }
}

#[derive(Debug, argh::FromArgs)]
#[argh(description = "a tool to build Rust projects using a Debian sysroot")]
struct Options {
    #[argh(option, description = "the Rust target triple")]
    target: String,

    #[argh(
        option,
        description = "the package to build",
        short = 'p',
        long = "package"
    )]
    package: String,

    #[argh(
        option,
        description = "the debian package to install",
        long = "install-package"
    )]
    install_package: Vec<String>,
}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();

    let metadata = MetadataCommand::new().exec()?;

    let target_dir = metadata.workspace_root.join("target");

    let sysroot_path = target_dir.join("debian-sysroot");
    let mut sysroot = DebianSysrootBuilder::new(sysroot_path.into()).build()?;

    let target = options.target.as_str();
    let package = options.package.as_str();

    let debian_arch = get_debian_arch(target)
        .with_context(|| format!("failed to get debian arch for \"{target}\""))?;
    let gcc_triple = get_gcc_triple(target)
        .with_context(|| format!("failed to get gcc triple for \"{target}\""))?;

    for package in options.install_package.iter() {
        let package = package.replace("%DEBIAN_ARCH%", debian_arch);
        println!("Installing {package}");

        sysroot.install(&package)?;
    }

    let sysroot = sysroot.get_sysroot_path();
    let sysroot = sysroot.to_str().context("sysroot path is not unicode")?;
    let cflags = format!("--sysroot {sysroot}/usr/{gcc_triple}");
    let rustflags = format!("-Clinker=clang -Clink-args=--target={target} -Clink-args=--sysroot={sysroot} -Clink-args=--gcc-toolchain={sysroot}/usr -Clink-args=-fuse-ld=lld");

    let output = Command::new("cargo")
        .current_dir(metadata.workspace_root.join("server"))
        .args(["build", "--release", "-p", package, "--target", target])
        .env("CC", "clang")
        .env("CFLAGS", cflags)
        .env("RUSTFLAGS", rustflags)
        .status()
        .context("failed to spawn command")?;
    ensure!(output.success(), "failed to run cargo build");

    Ok(())
}
