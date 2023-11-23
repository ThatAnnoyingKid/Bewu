use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use fd_lock::RwLock;
use flate2::read::GzDecoder;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use xz2::read::XzDecoder;

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

/// A builder for a debian sysroot.
///
/// # Resources
/// * https://wiki.debian.org/DebianRepository/Format
#[derive(Debug)]
pub struct DebianSysrootBuilder {
    /// The base path
    pub base_path: PathBuf,

    /// The repository url
    pub repository_url: String,

    /// The repository release
    pub release: String,

    /// The arch
    pub arch: String,
}

impl DebianSysrootBuilder {
    /// Create a new builder for a debian sysroot, at the given path.
    pub fn new(base_path: PathBuf) -> Self {
        const DEFAULT_REPOSITORY_URL: &str = "https://ftp.debian.org/debian";
        const DEFAULT_RELEASE: &str = "bookworm";
        const DEFAULT_ARCH: &str = "amd64";

        Self {
            base_path,
            repository_url: DEFAULT_REPOSITORY_URL.into(),
            release: DEFAULT_RELEASE.into(),
            arch: DEFAULT_ARCH.into(),
        }
    }

    /// Set the repository url.
    ///
    /// This defaults to https://ftp.debian.org/debian.
    pub fn repository_url<S>(&mut self, repository_url: S) -> &mut Self
    where
        S: Into<String>,
    {
        self.repository_url = repository_url.into();
        self
    }

    /// Set the repository release.
    ///
    /// This defaults to bookworm.
    pub fn release<S>(&mut self, release: S) -> &mut Self
    where
        S: Into<String>,
    {
        self.release = release.into();
        self
    }

    /// Set the repository arch.
    ///
    /// This defaults to amd64.
    pub fn arch<S>(&mut self, arch: S) -> &mut Self
    where
        S: Into<String>,
    {
        self.arch = arch.into();
        self
    }

    /// Build the debian sysroot.
    pub fn build(&mut self) -> anyhow::Result<DebianSysroot> {
        let path = self.base_path.join(&self.release).join(&self.arch);

        // TODO: Consider deferring, in the case that the provided release/arch is invalid.
        std::fs::create_dir_all(&path)?;
        std::fs::create_dir_all(path.join("sysroot"))?;
        std::fs::create_dir_all(path.join("packages"))?;

        let lock_path = path.join("lock");
        let lock = File::create(lock_path)?;
        let lock = RwLock::new(lock);

        let http = ureq::Agent::new();

        Ok(DebianSysroot {
            path,
            lock,
            http,
            repository_url: self.repository_url.clone(),
            release: self.release.clone(),
            arch: self.arch.clone(),
        })
    }
}

/// A debian sysroot
#[derive(Debug)]
pub struct DebianSysroot {
    path: PathBuf,
    lock: RwLock<File>,

    http: ureq::Agent,

    repository_url: String,
    release: String,
    arch: String,
}

impl DebianSysroot {
    /// Refresh the on-disk package list.
    pub fn update(&mut self) -> anyhow::Result<()> {
        let package_list_path = self.get_package_list_path();

        let write_lock = self.lock.write()?;

        if package_list_path.try_exists()? {
            drop(write_lock);
            return Ok(());
        }

        let packages_url = format!(
            "{}/dists/{}/main/binary-{}/Packages.gz",
            self.repository_url, self.release, self.arch
        );

        let response = self.http.get(&packages_url).call()?;
        ensure!(response.status() == 200);

        let response_reader = response.into_reader();
        let mut response_reader = GzDecoder::new(response_reader);
        let mut contents = String::new();
        response_reader.read_to_string(&mut contents)?;

        let tmp_file_path = self.path.join("Packages.txt.tmp");
        let mut tmp_file = File::create(&tmp_file_path)?;
        tmp_file.write_all(contents.as_bytes())?;
        tmp_file.flush()?;
        tmp_file.sync_all()?;
        drop(tmp_file);

        std::fs::rename(tmp_file_path, package_list_path)?;

        drop(write_lock);

        Ok(())
    }

    /// Install a package, if needed.
    pub fn install(&mut self, install_package_name: &str) -> anyhow::Result<()> {
        self.update()?;

        let package_list_path = self.get_package_list_path();

        let write_lock = self.lock.write()?;

        let downloaded_package_path = self
            .path
            .join("packages")
            .join(format!("{install_package_name}.deb"));
        if !downloaded_package_path.try_exists()? {
            let packages = std::fs::read_to_string(package_list_path)?;
            let mut package_file_name = None;
            for package in packages.trim().split("\n\n") {
                let name = package
                    .split('\n')
                    .find_map(|package| package.strip_prefix("Package: "))
                    .context("missing package name")?;

                if name == install_package_name {
                    package_file_name = Some(
                        package
                            .split('\n')
                            .find_map(|package| package.strip_prefix("Filename: "))
                            .context("missing package file name")?,
                    );
                }
            }
            let package_file_name = package_file_name.context("missing package")?;

            let deb_url = format!("{}/{}", self.repository_url, package_file_name);
            let response = self.http.get(&deb_url).call()?;
            ensure!(response.status() == 200);

            let mut response_reader = response.into_reader();

            let tmp_file_path = self
                .path
                .join("packages")
                .join(format!("{install_package_name}.deb.tmp"));

            let mut tmp_file = File::create(&tmp_file_path)?;
            std::io::copy(&mut response_reader, &mut tmp_file)?;
            tmp_file.flush()?;
            tmp_file.sync_all()?;

            std::fs::rename(&tmp_file_path, &downloaded_package_path)?;
        }

        let mut deb = ar::Archive::new(File::open(downloaded_package_path)?);
        let mut count = 0;
        while let Some(entry) = deb.next_entry() {
            let entry = entry?;
            let header = entry.header();
            let identifier = std::str::from_utf8(header.identifier())?;

            if count == 2 {
                let (_rest, extension) = identifier
                    .rsplit_once('.')
                    .context("expected an extension")?;

                let mut tar = match extension {
                    "xz" => {
                        let reader = XzDecoder::new(entry);
                        tar::Archive::new(reader)
                    }
                    _ => {
                        bail!("unknown extension \"{extension}\"");
                    }
                };

                // TODO: Do this is a way thats safer, esp during failure?
                let path = self.path.join("sysroot");
                for entry in tar.entries()? {
                    let mut entry = entry?;
                    let header = entry.header();

                    // This is a hack for windows.
                    if header.entry_type().is_symlink() {
                        let src = entry.link_name()?.context("missing link name")?;

                        // TODO: How to handle?
                        if !src.starts_with("/") {
                            continue;
                        }

                        let src = path.join(src.strip_prefix("/")?);
                        let dst = path.join(entry.path()?);

                        if let Some(parent) = dst.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::copy(&src, &dst).with_context(|| {
                            format!(
                                "failed to symlink-copy \"{}\" to \"{}\"",
                                src.display(),
                                dst.display()
                            )
                        })?;

                        continue;
                    }

                    entry.unpack_in(&path)?;
                }

                break;
            }

            count += 1;
        }

        drop(write_lock);

        Ok(())
    }

    /// Get a path to the package list.
    fn get_package_list_path(&self) -> PathBuf {
        self.path.join("Packages.txt")
    }

    /// Get the path to the sysroot.
    pub fn get_sysroot_path(&self) -> anyhow::Result<(impl Drop + '_, PathBuf)> {
        let read_lock = self.lock.read()?;
        let path = self.path.join("sysroot");

        Ok((read_lock, path))
    }
}
