use anyhow::ensure;
use anyhow::Context;
use std::net::SocketAddr;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug, serde::Deserialize)]
pub struct Config {
    #[serde(rename = "bind-address")]
    pub bind_address: SocketAddr,

    #[serde(rename = "public-directory")]
    pub public_directory: PathBuf,

    #[serde(rename = "data-directory")]
    pub data_directory: PathBuf,

    #[serde(default)]
    pub logging: ConfigLogging,
}

impl Config {
    /// Load and validate a config.
    pub fn load_path<P>(path: P) -> anyhow::Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("failed to load config file at \"{}\"", path.display()))?;
        let config: Self = toml::from_str(&data)
            .with_context(|| format!("failed to parse config file at \"{}\"", path.display()))?;

        let public_directory_exists = config.public_directory.try_exists().with_context(|| {
            format!(
                "failed to check if the public directory path \"{}\" exists",
                config.public_directory.display()
            )
        })?;
        ensure!(
            public_directory_exists,
            "the public directory path \"{}\" does not exist",
            config.public_directory.display()
        );

        Ok(config)
    }
}

#[derive(Debug, Default, serde::Deserialize)]
pub struct ConfigLogging {
    #[serde(rename = "include-headers", default)]
    pub include_headers: bool,

    #[serde(default)]
    pub directives: Vec<String>,
}
