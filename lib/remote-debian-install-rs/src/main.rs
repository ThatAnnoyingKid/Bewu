use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use cargo_metadata::MetadataCommand;
use libssh_rs::OpenFlags;
use rand::distributions::DistString;
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
        let random_part = rand::distributions::Alphanumeric.sample_string(&mut rng, 12);
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
        hasher.finish()
    };
    let mut temp_dir =
        TempDir::new("remote-debian-install", hash).context("failed to create temp dir")?;
    // Only delete on success.
    temp_dir.set_persist(true);

    // TODO: Get from remote
    let target = "x86_64-unknown-linux-gnu";

    // TODO: Consider embedding a git client
    if !temp_dir.path().join(".git").try_exists()? {
        let status = Command::new("git")
            .current_dir(temp_dir.path())
            .args(["clone", &options.git, "."])
            .status()?;
        ensure!(status.success());
    }

    let cargo_metadata = MetadataCommand::new().current_dir(temp_dir.path()).exec()?;
    let workspace_packages = cargo_metadata.workspace_packages();
    let bin_name = match (workspace_packages.len(), options.package.as_deref()) {
        (0, _) => bail!("workspace has no members"),
        (1, None) => workspace_packages[0].name.clone(),
        (_, None) => {
            bail!("more than one package, need to specify package name");
        }
        (_n, Some(package_name)) => {
            let has_package = workspace_packages
                .iter()
                .any(|package| package.name == package_name);
            ensure!(has_package, "no package with name \"{package_name}\"");

            package_name.to_string()
        }
    };

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
