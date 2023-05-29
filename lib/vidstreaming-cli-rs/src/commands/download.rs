use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tokio_stream::StreamExt;
use url::Url;

#[derive(Debug, argh::FromArgs)]
#[argh(subcommand, name = "download", description = "download a single video")]
pub struct Options {
    #[argh(positional, description = "the url of the episode to download")]
    pub url: Url,

    #[argh(
        option,
        description = "the source the download should use. Defaults to main.",
        default = "Default::default()"
    )]
    pub source: Source,
}

/// The source
#[derive(Debug, Default)]
pub enum Source {
    #[default]
    Main,
    Backup,
}

impl std::str::FromStr for Source {
    type Err = anyhow::Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "main" => Ok(Self::Main),
            "backup" => Ok(Self::Backup),
            _ => bail!("unknown source \"{input}\""),
        }
    }
}

pub async fn exec(client: vidstreaming::Client, options: Options) -> anyhow::Result<()> {
    println!("Fetching episode page...");
    let episode = client
        .get_episode(options.url.as_str())
        .await
        .with_context(|| format!("failed to get episode for \"{}\"", options.url.as_str()))?;

    let out_path = PathBuf::new().join(format!("{}.mp4", episode.name));

    if tokio::fs::try_exists(&out_path).await? {
        println!("File exists, exiting...");
        return Ok(());
    }

    println!("Fetching video player...");
    let video_player = client
        .get_video_player(episode.video_player_url.as_str())
        .await
        .with_context(|| {
            format!(
                "failed to get video player for \"{}\"",
                episode.video_player_url.as_str()
            )
        })?;
    println!("Fetching video player video data...");
    let video_player_video_data = client
        .get_video_player_video_data(&video_player)
        .await
        .context("failed to get video player video data")?;
    let best_source = match options.source {
        Source::Main => video_player_video_data
            .get_best_source()
            .context("failed to select source")?,
        Source::Backup => {
            ensure!(video_player_video_data.source_bk.len() == 1);
            video_player_video_data
                .source_bk
                .first()
                .context("failed to select source")?
        }
    };

    ensure!(
        best_source.is_hls(),
        "the selected source is not a HLS stream"
    );

    download_hls_to_mp4(&client.client, best_source.file.as_str(), &out_path).await?;

    Ok(())
}

// TODO: Consider making a util func
#[allow(dead_code)]
pub async fn probe_url(url: &str) -> anyhow::Result<ProbeResult> {
    let output = tokio::process::Command::new("ffprobe")
        .args(["-v", "error"])
        .arg("-hide_banner")
        .arg(url)
        .args(["-of", "default=noprint_wrappers=0"])
        .args(["-print_format", "json"])
        .arg("-show_format")
        // .args(["-show_entries", "stream"])
        // .arg("-show_programs")
        .output()
        .await
        .context("failed to spawn \"ffprobe\"")?;

    ensure!(
        output.status.success(),
        "ffprobe exited with \"{}\"",
        output.status
    );

    let stdout = std::str::from_utf8(&output.stdout)?;

    Ok(serde_json::from_str(stdout)?)
}

/// Result of probing
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct ProbeResult {
    /// Format info
    pub format: Format,

    #[serde(flatten)]
    pub unknown: HashMap<String, serde_json::Value>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct Format {
    /// The # of programs
    pub nb_programs: u32,

    /// The start time, as a string?
    pub start_time: String,

    /// The file name
    pub filename: String,

    /// The bit rate, as a string?
    pub bit_rate: String,

    /// The format name
    pub format_name: String,

    /// The # of streams, as a string
    pub nb_streams: u32,

    /// The long name of the format
    pub format_long_name: String,

    /// The size of the data?
    pub size: String,

    /// The duration, as a string?
    pub duration: String,

    /// ?
    pub probe_score: u32,

    /// Extra k/v
    #[serde(flatten)]
    pub unknown: HashMap<String, serde_json::Value>,
}

/// Download a hls stream.
async fn download_hls_to_mp4<P>(client: &reqwest::Client, url: &str, path: P) -> anyhow::Result<()>
where
    P: AsRef<Path>,
{
    let path = path.as_ref();

    if tokio::fs::try_exists(&path).await? {
        return Ok(());
    }

    let stream = bewu_util::download_hls(client.clone(), url, path)?;
    tokio::pin!(stream);

    let mut stream_duration: Option<Duration> = None;
    let mut download_progress_bar = None;
    let mut concat_progress_bar = None;
    let mut remux_progress_bar = None;
    while let Some(message) = stream.next().await {
        let message = match message {
            Ok(message) => message,
            Err(error) => {
                return Err(error);
            }
        };
        match message {
            bewu_util::DownloadHlsMessage::DownloadedMediaPlaylist { media_playlist } => {
                let total = media_playlist.media_segments.len();
                let progress_bar = indicatif::ProgressBar::new(u64::try_from(total)?);
                let progress_bar_style_template = "[Time = {elapsed_precise} | ETA = {eta_precise}] Downloading media segments {wide_bar}";
                let progress_bar_style = indicatif::ProgressStyle::default_bar()
                    .template(progress_bar_style_template)
                    .expect("invalid progress bar style template");
                progress_bar.set_style(progress_bar_style);

                download_progress_bar = Some(progress_bar);

                stream_duration = Some(
                    media_playlist
                        .media_segments
                        .iter()
                        .map(|segment| segment.duration)
                        .sum(),
                );
            }
            bewu_util::DownloadHlsMessage::DownloadedMediaSegment => {
                if let Some(progress_bar) = download_progress_bar.as_ref() {
                    progress_bar.inc(1);
                }
            }
            bewu_util::DownloadHlsMessage::DownloadedAllMediaSegments => {
                if let Some(progress_bar) = download_progress_bar.as_ref() {
                    progress_bar.finish();

                    let progress_bar =
                        indicatif::ProgressBar::new(progress_bar.length().unwrap_or(0));
                    let progress_bar_style_template =
                        "[Time = {elapsed_precise} | ETA = {eta_precise}] Concatenating media segments {wide_bar}";
                    let progress_bar_style = indicatif::ProgressStyle::default_bar()
                        .template(progress_bar_style_template)
                        .expect("invalid progress bar style template");
                    progress_bar.set_style(progress_bar_style);
                    concat_progress_bar = Some(progress_bar);
                }
            }
            bewu_util::DownloadHlsMessage::ConcatenatedMediaSegment => {
                if let Some(progress_bar) = concat_progress_bar.as_ref() {
                    progress_bar.inc(1);
                }
            }
            bewu_util::DownloadHlsMessage::ConcatenatedAllMediaSegments => {
                if let Some(progress_bar) = concat_progress_bar.as_ref() {
                    progress_bar.finish();
                }

                if let Some(stream_duration) = stream_duration {
                    let progress_bar = indicatif::ProgressBar::new(stream_duration.as_secs());
                    let progress_bar_style_template =
                        "[Time = {elapsed_precise} | ETA = {eta_precise}] Remuxing {wide_bar}";
                    let progress_bar_style = indicatif::ProgressStyle::default_bar()
                        .template(progress_bar_style_template)
                        .expect("invalid progress bar style template");
                    progress_bar.set_style(progress_bar_style);

                    remux_progress_bar = Some(progress_bar);
                }
            }
            bewu_util::DownloadHlsMessage::FfmpegProgress { out_time } => {
                if let Some(progress_bar) = remux_progress_bar.as_ref() {
                    progress_bar.set_position(out_time);
                }
            }
            bewu_util::DownloadHlsMessage::Done => {
                if let Some(progress_bar) = remux_progress_bar.as_ref() {
                    progress_bar.finish();
                }
            }
        }
    }

    Ok(())
}
