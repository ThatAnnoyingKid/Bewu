use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use flate2::read::GzDecoder;
use rusqlite::named_params;
use rusqlite::OptionalExtension;
use std::collections::HashSet;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use xz2::read::XzDecoder;

const SETUP_SQL: &str = "
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS packages (
    name TEXT NOT NULL PRIMARY KEY,
    file_name TEXT NOT NULL,
    depends TEXT
) STRICT;

CREATE TABLE IF NOT EXISTS installed_packages (
    name TEXT NOT NULL PRIMARY KEY,
    file_name TEXT NOT NULL,
    depends TEXT
) STRICT;
";

fn symlink_dir<S: AsRef<Path>, D: AsRef<Path>>(src: S, dest: D) -> anyhow::Result<()> {
    let src = src.as_ref();
    let dest = dest.as_ref();

    cfg_if::cfg_if! {
        if #[cfg(windows)] {
            use std::os::windows::fs::symlink_dir;
            symlink_dir(src, dest)?;

            Ok(())
        } else if #[cfg(unix)] {
            use std::os::unix::fs::symlink;
            symlink(src, dest)?;

            Ok(())
        } else {
            bail!("symlinks are not supported on this platform");
        }
    }
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
        let sysroot = path.join("sysroot");
        std::fs::create_dir_all(&path)?;
        std::fs::create_dir_all(&sysroot)?;
        std::fs::create_dir_all(path.join("packages"))?;

        let lock_path = path.join("lock");
        let mut lock = fslock::LockFile::open(&lock_path)?;
        lock.lock()?;

        let database = rusqlite::Connection::open(path.join("database.db"))?;
        database.execute_batch(SETUP_SQL)?;

        // Setup usrmerge if on trixie.
        //
        // The usrmerge package doesn't work on Windows due to needing perl,
        // which needs ":" in paths.
        if self.release.as_str() == "trixie" {
            let usr = sysroot.join("usr");
            let lib = sysroot.join("lib");

            std::fs::create_dir_all(&usr)?;

            symlink_dir(usr.join("lib"), lib)?;
        }

        let http = ureq::agent();
        Ok(DebianSysroot {
            path,
            lock,

            database,
            http,

            repository_url: self.repository_url.clone(),
            release: self.release.clone(),
            arch: self.arch.clone(),

            pending_install: HashSet::new(),
        })
    }
}

/// A debian sysroot
#[derive(Debug)]
pub struct DebianSysroot {
    path: PathBuf,
    lock: fslock::LockFile,

    database: rusqlite::Connection,
    http: ureq::Agent,

    repository_url: String,
    release: String,
    arch: String,

    pending_install: HashSet<String>,
}

impl DebianSysroot {
    /// Refresh the on-disk package list.
    pub fn update(&mut self) -> anyhow::Result<()> {
        let package_list_path = self.path.join("Packages.txt");

        if package_list_path.try_exists()? {
            return Ok(());
        }

        let packages_url = format!(
            "{}/dists/{}/main/binary-{}/Packages.gz",
            self.repository_url, self.release, self.arch
        );

        let mut response = self.http.get(&packages_url).call()?;
        ensure!(response.status() == 200);

        let response_reader = response.body_mut().as_reader();
        let mut response_reader = GzDecoder::new(response_reader);
        let mut contents = String::new();
        response_reader.read_to_string(&mut contents)?;

        let transaction = self.database.transaction()?;
        {
            let mut statement = transaction.prepare_cached(
                "
                INSERT OR REPLACE INTO packages (
                    name,
                    file_name,
                    depends
                ) VALUES (
                    :name,
                    :file_name,
                    :depends
                );
                ",
            )?;
            for package in parse_package_list(&contents) {
                let package = package?;

                let depends = if package.depends.is_empty() {
                    None
                } else {
                    Some(package.depends.join(", "))
                };

                statement.execute(named_params! {
                    ":name": package.name,
                    ":file_name": package.file_name,
                    ":depends": depends,
                })?;
            }
        }

        transaction.commit()?;

        File::create(package_list_path)?;

        Ok(())
    }

    /// Get the info for a given package by name.
    fn get_package_info(&mut self, name: &str) -> anyhow::Result<Option<PackageInfo>> {
        let mut statement = self.database.prepare_cached(
            "
            SELECT
                name,
                file_name, 
                depends
            FROM
                packages
            where
                name = :name;
        ",
        )?;
        statement
            .query_row(
                named_params! {
                    ":name": name,
                },
                |row| {
                    let name = row.get("name")?;
                    let file_name = row.get("file_name")?;
                    let depends: Option<String> = row.get("depends")?;

                    let depends = match depends {
                        Some(depends) => depends.split(", ").map(Into::into).collect(),
                        None => Vec::new(),
                    };

                    Ok(anyhow::Ok(PackageInfo {
                        name,
                        file_name,
                        depends,
                    }))
                },
            )
            .optional()?
            .transpose()
    }

    /// Get the installed package info.
    fn get_installed_package_info(&mut self, name: &str) -> anyhow::Result<Option<PackageInfo>> {
        let mut statement = self.database.prepare_cached(
            "
            SELECT
                name,
                file_name, 
                depends
            FROM
                installed_packages
            where
                name = :name;
        ",
        )?;
        statement
            .query_row(
                named_params! {
                    ":name": name,
                },
                |row| {
                    let name = row.get("name")?;
                    let file_name = row.get("file_name")?;
                    let depends: String = row.get("depends")?;

                    let depends = depends.split(", ").map(Into::into).collect();

                    Ok(anyhow::Ok(PackageInfo {
                        name,
                        file_name,
                        depends,
                    }))
                },
            )
            .optional()?
            .transpose()
    }

    /// Install a package, if needed.
    pub fn install(&mut self, install_package_name: &str) -> anyhow::Result<()> {
        if self
            .get_installed_package_info(install_package_name)?
            .is_some()
        {
            return Ok(());
        }

        if !self.pending_install.insert(install_package_name.into()) {
            return Ok(());
        }

        self.update()?;

        let package = self
            .get_package_info(install_package_name)?
            .with_context(|| format!("missing package \"{install_package_name}\""))?;

        for dep in package.depends.iter() {
            let package = dep.split_once(' ').map(|(name, _rest)| name).unwrap_or(dep);
            self.install(package)?;
        }

        let downloaded_package_path = self
            .path
            .join("packages")
            .join(format!("{install_package_name}.deb"));
        if !downloaded_package_path.try_exists()? {
            let deb_url = format!("{}/{}", self.repository_url, package.file_name);
            let mut response = self.http.get(&deb_url).call()?;
            ensure!(response.status() == 200);

            let tmp_file_path = self
                .path
                .join("packages")
                .join(format!("{install_package_name}.deb.tmp"));

            let mut tmp_file = File::create(&tmp_file_path)?;
            std::io::copy(&mut response.body_mut().as_reader(), &mut tmp_file)?;
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
                        let dst = path.join(entry.path()?);

                        let src = match src.strip_prefix("/") {
                            Ok(src) => src.to_owned(),
                            Err(_err) => dst.parent().context("dst missing parent")?.join(src),
                        };
                        let src = path.join(src);

                        if let Some(parent) = dst.parent() {
                            std::fs::create_dir_all(parent)?;
                        }

                        let src_metadata = std::fs::metadata(&src).with_context(|| {
                            format!("failed to get metadata for file at {}", src.display())
                        })?;

                        // TODO: Copy Dir
                        if src_metadata.is_dir() {
                            continue;
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

        self.database
            .prepare_cached(
                "
                    INSERT OR REPLACE INTO installed_packages (
                        name,
                        file_name,
                        depends
                    ) VALUES (
                        :name, 
                        :file_name,
                        :depends
                    );
                
                ",
            )?
            .execute(named_params! {
                ":name": package.name,
                ":file_name": package.file_name,
                ":depends": package.depends.join(", "),
            })?;

        Ok(())
    }

    /// Get the base path.
    pub fn get_base_path(&self) -> &Path {
        &self.path
    }

    /// Get the path to the sysroot.
    pub fn get_sysroot_path(&self) -> PathBuf {
        self.path.join("sysroot")
    }
}

impl Drop for DebianSysroot {
    fn drop(&mut self) {
        let _ = self.lock.unlock().is_ok();
    }
}

/// Package info
#[derive(Debug)]
pub struct PackageInfo {
    /// The name of the package
    pub name: String,

    /// The package file name
    pub file_name: String,

    /// Package dependencies
    pub depends: Vec<String>,
}

/// Parse a package list.
pub fn parse_package_list(input: &str) -> impl Iterator<Item = anyhow::Result<PackageInfo>> + '_ {
    input.trim_end().split("\n\n").map(|package| {
        let mut name = None;
        let mut file_name = None;
        let mut depends = None;
        for line in package.split('\n') {
            // TODO: Multiline support
            let (key, value) = match line.split_once(": ") {
                Some(value) => value,
                None => {
                    continue;
                }
            };

            match key {
                "Package" => {
                    ensure!(name.is_none());
                    name = Some(value);
                }
                "Filename" => {
                    ensure!(file_name.is_none());
                    file_name = Some(value);
                }
                "Depends" => {
                    ensure!(depends.is_none());
                    depends = Some(value);
                }
                _ => {}
            }
        }
        let name = name.context("missing package name")?.into();
        let file_name = file_name.context("missing package file name")?.into();
        let depends = depends
            .map(|depends| depends.split(", ").map(Into::into).collect())
            .unwrap_or(Vec::new());

        Ok(PackageInfo {
            name,
            file_name,
            depends,
        })
    })
}
