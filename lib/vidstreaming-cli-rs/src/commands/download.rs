use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use bewu_util::parse_ffmpeg_time;
use hls_parser::MasterPlaylist;
use hls_parser::MediaPlaylist;
use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;
use tokio::task::JoinSet;
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
    let temp_dir = nd_util::with_push_extension(path, "part.dir");
    let temp_path = nd_util::with_push_extension(path, "part");

    if tokio::fs::try_exists(&path).await? {
        return Ok(());
    }

    let playlist_text = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let playlist: MasterPlaylist = playlist_text.parse().context("failed to parse playlist")?;
    let best_variant_stream = playlist
        .variant_streams
        .iter()
        .max_by_key(|stream| stream.bandwidth)
        .context("failed to select a variant stream")?;

    let playlist_uri = match best_variant_stream.uri.to_iri() {
        Ok(absolute_uri) => Url::parse(absolute_uri.into())?,
        Err(relative_uri) => {
            let url = Url::parse(url)?;
            Url::options()
                .base_url(Some(&url))
                .parse(relative_uri.into())?
        }
    };

    let playlist_text = client
        .get(playlist_uri.as_str())
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let playlist: MediaPlaylist = playlist_text.parse()?;

    tokio::fs::create_dir_all(&temp_dir).await?;

    let mut join_set = JoinSet::new();
    let mut segment_paths = Vec::with_capacity(playlist.media_segments.len());
    for segment in playlist.media_segments.iter() {
        // We only support mgpeg2-ts streams for now
        ensure!(segment.uri.as_str().ends_with(".ts"));

        let client = client.clone();

        let url = match segment.uri.to_iri() {
            Ok(absolute_uri) => Url::parse(absolute_uri.into())?,
            Err(relative_uri) => Url::options()
                .base_url(Some(&playlist_uri))
                .parse(relative_uri.into())?,
        };
        // Generated to be unique for segment uri.
        let file_name = url_to_file_name(url.as_str());
        let out_path = temp_dir.join(file_name);

        segment_paths.push(out_path.clone());

        join_set.spawn(async move {
            if tokio::fs::try_exists(&out_path).await? {
                return Ok(());
            }

            nd_util::download_to_path(&client, url.as_str(), out_path).await
        });
    }

    let total = playlist.media_segments.len();
    let progress_bar = indicatif::ProgressBar::new(total as u64);
    let progress_bar_style_template =
        "[Time = {elapsed_precise} | ETA = {eta_precise}] Downloading segments {wide_bar}";
    let progress_bar_style = indicatif::ProgressStyle::default_bar()
        .template(progress_bar_style_template)
        .expect("invalid progress bar style template");
    progress_bar.set_style(progress_bar_style);

    let mut last_error: anyhow::Result<()> = Ok(());
    while let Some(result) = join_set.join_next().await {
        let result = result
            .context("failed to join task")
            .and_then(std::convert::identity);
        progress_bar.inc(1);
        match result {
            Ok(()) => {}
            Err(error) => {
                last_error = Err(error);
            }
        }
    }
    progress_bar.finish();
    last_error?;

    // TODO: May be asyncified by stat-ing all component files,
    // calculating offsets,
    // then writing to different segments of the file concurrently.
    // This may even be done as part of the download process, avoiding the need of a second copy.
    // However, if the server does not send a content-length header, we must download everything to a seperate file.
    // A temp dir of some kind will always be required.
    // Copy to intermediate ts file
    let concat_file_name = url_to_file_name(playlist_uri.as_str());
    let concat_file_path = temp_dir.join(&concat_file_name);
    {
        use std::fs::File;
        use std::io::Write;

        let concat_file_path = concat_file_path.clone();

        tokio::task::spawn_blocking(|| {
            let dest_file = File::create(concat_file_path)?;
            let mut dest_file = fd_lock::RwLock::new(dest_file);
            {
                let mut dest_file = dest_file.write()?;

                for path in segment_paths {
                    let mut src_file = File::open(path)?;
                    std::io::copy(&mut src_file, &mut *dest_file)?;
                }
            }
            let mut dest_file = dest_file.into_inner();
            dest_file.flush()?;
            dest_file.sync_all()?;

            Result::<_, anyhow::Error>::Ok(())
        })
        .await??;
    }

    let mut concat_stream = tokio_ffmpeg_cli::Builder::new()
        .audio_codec("copy")
        .video_codec("copy")
        .input(concat_file_path)
        .output_format("mp4")
        .output(&temp_path)
        .overwrite(false)
        .spawn()?;

    let duration: Duration = playlist
        .media_segments
        .iter()
        .map(|segment| segment.duration)
        .sum();
    let progress_bar = indicatif::ProgressBar::new(duration.as_secs());
    let progress_bar_style_template =
        "[Time = {elapsed_precise} | ETA = {eta_precise}] Remuxing {wide_bar}";
    let progress_bar_style = indicatif::ProgressStyle::default_bar()
        .template(progress_bar_style_template)
        .expect("invalid progress bar style template");
    progress_bar.set_style(progress_bar_style);

    let mut last_error = Ok(());
    while let Some(event) = concat_stream.next().await {
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
            tokio::fs::rename(&temp_path, path).await?;
            tokio::fs::remove_dir_all(&temp_dir).await?;
        }
        Err(_e) => {}
    }
    last_error
}

fn url_to_file_name(url: &str) -> String {
    let mut file_name = String::with_capacity(url.len());
    for c in url.chars() {
        match c {
            '\\' | '/' | ':' | 'x' => {
                let c = u32::from(c);
                write!(file_name, "x{c:02X}").unwrap();
            }
            c => {
                file_name.push(c);
            }
        }
    }

    file_name
}
