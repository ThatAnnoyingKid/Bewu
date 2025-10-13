use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use cargo_metadata::MetadataCommand;
use libssh_rs::OpenFlags;
use rand::distr::SampleString;
use rand::SeedableRng;
use rand_chacha::ChaCha12Rng;
use seahash::SeaHasher;
use std::fs::File;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

const GET_CARGO_HOME_SH: &str = "
if [[ -z \"${CARGO_HOME}\" ]]; then
  echo $HOME/.cargo
else
  echo $CARGO_HOME
fi
";

#[derive(Debug, argh::FromArgs)]
#[argh(description = "a remote debian-only version of `cargo install`")]
struct Options {
    #[argh(
        option,
        long = "git",
        description = "the git repository to install from"
    )]
    git: String,

    #[argh(
        option,
        long = "branch",
        description = "the git branch to install from"
    )]
    branch: Option<String>,

    #[argh(
        option,
        long = "package",
        short = 'p',
        description = "the package to install"
    )]
    package: Option<String>,

    #[argh(option, long = "remote", description = "the remote to install to")]
    remote: String,
}

struct TempDir {
    path: PathBuf,
    persist: bool,
}

impl TempDir {
    /// Make a new temp dir.
    pub fn new(name: &str, hash: u64) -> anyhow::Result<Self> {
        let temp_dir = std::env::temp_dir();
        let mut rng = ChaCha12Rng::seed_from_u64(hash);
        let random_part = rand::distr::Alphanumeric.sample_string(&mut rng, 12);
        let path = temp_dir.join(format!("{name}-{random_part}"));

        match std::fs::create_dir(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error.into());
            }
        }

        Ok(Self {
            path,
            persist: false,
        })
    }

    /// Get the path
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Set whether this will persist.
    pub fn set_persist(&mut self, persist: bool) {
        self.persist = persist;
    }

    /// Delete this dir, if not persisted.
    pub fn close(self) -> Result<(), anyhow::Error> {
        if !self.persist {
            std::fs::remove_dir_all(&self.path).context("failed to delete temp dir")?;
        }

        Ok(())
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.persist {
            let _ = std::fs::remove_dir_all(&self.path).is_ok();
        }
    }
}

fn main() -> anyhow::Result<()> {
    let options: Options = argh::from_env();

    let hash = {
        let mut hasher = SeaHasher::new();
        options.git.hash(&mut hasher);
        hasher.write_u8(0x12);
        options.branch.hash(&mut hasher);
        hasher.write_u8(0x12);
        hasher.finish()
    };
    let mut temp_dir =
        TempDir::new("remote-debian-install", hash).context("failed to create temp dir")?;
    // Only delete on success.
    temp_dir.set_persist(true);

    // TODO: Validate that the host arch doesn't change while we build
    let dpkg_arch = {
        let session = deploy_deb::init_ssh_session(&options.remote)?;

        {
            let channel = session.new_channel()?;
            channel.open_session()?;
            channel.request_exec("dpkg --print-architecture")?;
            channel.send_eof()?;

            let mut buffer = Vec::new();
            std::io::copy(&mut channel.stdout(), &mut buffer)?;

            // Remove /n
            assert!(buffer.ends_with(b"\n"));
            buffer.pop();

            let exit_status = channel.get_exit_status().context("missing exit status")?;
            ensure!(exit_status == 0, "invalid exit status {exit_status}");

            String::from_utf8(buffer)?
        }
    };
    let target = match dpkg_arch.as_str() {
        "arm64" => "aarch64-unknown-linux-gnu",
        "amd64" => "x86_64-unknown-linux-gnu",
        _ => bail!("unknown dpkg arch \"{dpkg_arch}\""),
    };

    // TODO: Consider embedding a git client
    let git_dir = temp_dir.path().join(".git");
    let git_head = git_dir.join("HEAD");
    let git_dir_exists = git_dir.try_exists()?;
    let git_head_exists = git_dir_exists && git_head.try_exists()?;
    match (git_dir_exists, git_head_exists) {
        // If the git dir exists and it seems like a valid git repo, just pull.
        (true, true) => {
            let path = temp_dir.path();

            let mut command = Command::new("git");
            command.current_dir(path).arg("pull");
            let status = command.status()?;
            ensure!(
                status.success(),
                "failed to \"git pull\" \"{}\"",
                path.display()
            );
        }
        // On Windows, it appears that the storage cleaner appers to only delete temp files, not folders.
        // This leads to a scenario where the git folder may exist but the files may not.
        // If the git HEAD does not exist, delete the entire folder and re-clone.
        (true, false) => {
            let path = temp_dir.path();

            std::fs::remove_dir_all(path)
                .with_context(|| format!("failed to delete folder \"{}\"", path.display()))?;
            std::fs::create_dir(path)
                .with_context(|| format!("failed to delete folder \"{}\"", path.display()))?;

            git_clone(path, &options.git, options.branch.as_deref())?;
        }
        // The git dir does not exist, just re-clone.
        (false, _) => {
            let path = temp_dir.path();

            git_clone(path, &options.git, options.branch.as_deref())?;
        }
    }

    let cargo_metadata = MetadataCommand::new().current_dir(temp_dir.path()).exec()?;
    let workspace_packages = cargo_metadata.workspace_packages();
    let package = match (workspace_packages.len(), options.package.as_deref()) {
        (0, _) => bail!("workspace has no members"),
        (1, None) => &workspace_packages[0],
        (_, None) => {
            bail!("more than one package, need to specify package name");
        }
        (_n, Some(package_name)) => workspace_packages
            .iter()
            .find(|package| package.name == package_name)
            .with_context(|| format!("no package with name \"{package_name}\""))?,
    };
    // TODO: Account for multiple bins
    let bin_name = package
        .targets
        .iter()
        .find(|target| target.is_bin())
        .context("package does not have a bin target")?
        .name
        .clone();

    // TODO: Consider embedding deploy-deb
    let mut command = Command::new("debian-sysroot-build");
    command
        .current_dir(temp_dir.path())
        .args(["--target", target]);
    if let Some(package) = options.package.as_deref() {
        command.args(["--package", package]);
    }
    let status = command
        .args([
            "--install-package",
            "libc6",
            "--install-package",
            "libc6-dev",
            "--install-package",
            "linux-libc-dev",
            "--install-package",
            "libgcc-12-dev",
        ])
        .status()?;
    ensure!(status.success());

    let local_bin_path = temp_dir
        .path
        .join(format!("target/{target}/release/{bin_name}"));
    let session = deploy_deb::init_ssh_session(&options.remote)?;

    let cargo_home = {
        let channel = session.new_channel()?;
        channel.open_session()?;
        channel.request_exec(GET_CARGO_HOME_SH)?;
        channel.send_eof()?;

        let mut buffer = Vec::new();
        std::io::copy(&mut channel.stdout(), &mut buffer)?;

        // Remove /n
        assert!(buffer.ends_with(b"\n"));
        buffer.pop();

        let exit_status = channel.get_exit_status().context("missing exit status")?;
        ensure!(exit_status == 0, "invalid exit status {exit_status}");

        String::from_utf8(buffer)?
    };

    let remote_bin_path = format!("{cargo_home}/bin/{bin_name}");

    {
        println!("Opening SFTP channel...");
        let sftp = session.sftp().context("failed to open sftp channel")?;

        println!("Uploading bin...");
        let local_bin_file = File::open(&local_bin_path)
            .with_context(|| format!("failed to open \"{}\"", local_bin_path.display()))?;
        let local_bin_file_metadata = local_bin_file.metadata()?;

        let mut remote_bin_file = sftp
            .open(
                remote_bin_path.as_str(),
                OpenFlags::WRITE_ONLY | OpenFlags::CREATE,
                0o600, // Prevent users from tampering with the file.
            )
            .with_context(|| format!("failed to open \"{remote_bin_path}\" on the server"))?;

        // Perform copy
        let metadata_len = local_bin_file_metadata.len();
        let progress_bar = indicatif::ProgressBar::new(metadata_len);
        let progress_bar_style_template = "[Time = {elapsed_precise} | ETA = {eta_precise} | Speed = {bytes_per_sec}] {wide_bar} {bytes}/{total_bytes}";
        let progress_bar_style = indicatif::ProgressStyle::default_bar()
            .template(progress_bar_style_template)
            .expect("invalid progress bar style template");
        progress_bar.set_style(progress_bar_style);

        let bytes_copied = std::io::copy(
            &mut progress_bar.wrap_read(local_bin_file),
            &mut remote_bin_file,
        )?;
        progress_bar.finish();
        ensure!(
            metadata_len == bytes_copied,
            "file length changed during transfer, (expected) {metadata_len} != (actual) {bytes_copied}",
        );
        remote_bin_file.flush()?;
    }

    println!("Making bin executable...");
    {
        let channel = session.new_channel()?;
        channel.open_session()?;
        channel.request_exec(&format!("chmod +x {remote_bin_path}"))?;
        channel.send_eof()?;

        {
            let mut stdout_lock = std::io::stdout();
            std::io::copy(&mut channel.stdout(), &mut stdout_lock)?;
        }

        let exit_status = channel.get_exit_status().context("missing exit status")?;
        ensure!(exit_status == 0, "invalid exit status {exit_status}");
    }

    temp_dir.close()?;

    Ok(())
}

fn git_clone(path: &Path, src: &str, branch: Option<&str>) -> anyhow::Result<()> {
    let mut command = Command::new("git");
    command.current_dir(path).arg("clone");
    if let Some(branch) = branch {
        command.args(["--branch", branch]);
    }
    command.args([src, "."]);
    let status = command
        .status()
        .context("failed to spawn `git clone` command")?;
    ensure!(
        status.success(),
        "failed to `git clone` to \"{}\"",
        path.display()
    );

    Ok(())
}
