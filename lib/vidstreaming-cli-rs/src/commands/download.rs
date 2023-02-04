use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use std::collections::HashMap;
use std::path::PathBuf;
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
    source: Source,
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

    if bewu_util::try_exists(&out_path).await? {
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

    println!("Probing sources...");
    let probe_result = probe_url(best_source.file.as_str()).await?;
    let duration: f64 = probe_result
        .format
        .duration
        .parse::<f64>()
        .context("failed to parse duration")?
        .floor();
    println!("  Duration: {duration}");

    let temp_path = nd_util::with_push_extension(&out_path, "part");

    println!("Starting download stream...");
    let mut download_stream = tokio_ffmpeg_cli::Builder::new()
        .audio_codec("copy")
        .video_codec("copy")
        .input(best_source.file.as_str())
        .output_format("mp4")
        .output(&temp_path)
        .overwrite(false)
        .spawn()?;

    let progress_bar = indicatif::ProgressBar::new(duration as u64);
    let progress_bar_style_template = "[Time = {elapsed_precise} | ETA = {eta_precise}] {wide_bar}";
    let progress_bar_style = indicatif::ProgressStyle::default_bar()
        .template(progress_bar_style_template)
        .expect("invalid progress bar style template");
    progress_bar.set_style(progress_bar_style);

    let mut last_error = Ok(());
    while let Some(event) = download_stream.next().await {
        let event = match event {
            Ok(event) => event,
            Err(e) => {
                last_error = Err(e).context("stream event error");
                continue;
            }
        };

        match event {
            tokio_ffmpeg_cli::Event::Progress(event) => {
                let out_time = match parse_ffmpeg_time(&event.out_time) {
                    Ok(out_time) => out_time,
                    Err(e) => {
                        last_error = Err(e).context("failed to parse out time");
                        continue;
                    }
                };

                progress_bar.set_position(out_time);
            }
            tokio_ffmpeg_cli::Event::ExitStatus(exit_status) => {
                if !exit_status.success() {
                    last_error = Err(anyhow!("ffmpeg exit status was \"{exit_status:?}\""));
                }
            }
            tokio_ffmpeg_cli::Event::Unknown(_line) => {
                // dbg!(line);
                // Data that was not parsed, probably only useful for debugging.
            }
        }
    }
    progress_bar.finish();

    match last_error.as_ref() {
        Ok(()) => {
            tokio::fs::rename(&temp_path, &out_path).await?;
        }
        Err(_e) => {
            if let Err(e) = tokio::fs::remove_file(temp_path)
                .await
                .context("failed to remove temp file")
            {
                eprintln!("{e}");
            }
        }
    }

    last_error
}

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

/// Parse an ffmpeg time, returning the time in seconds.
///
/// Format: `00:00:03.970612`, `HOURS:MM:SS.MICROSECONDS`
fn parse_ffmpeg_time(time: &str) -> anyhow::Result<u64> {
    let mut iter = time.split(':');

    let hours: u64 = iter.next().context("missing hours")?.parse()?;
    let minutes: u64 = iter.next().context("missing minutes")?.parse()?;

    let seconds = iter.next().context("missing seconds")?;
    let (seconds, microseconds) = seconds
        .split_once('.')
        .context("failed to separate seconds and microseconds")?;
    let _microseconds: u64 = microseconds.parse()?;
    let seconds: u64 = seconds.parse()?;

    ensure!(iter.next().is_none());

    Ok((hours * 60 * 60) + (minutes * 60) + seconds)
}
