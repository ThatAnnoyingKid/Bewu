use crate::AsyncLockFile;
use anyhow::anyhow;
use anyhow::Context;
use hls_parser::MasterPlaylist;
use hls_parser::MediaPlaylist;
use reqwest::Url;
use std::fmt::Write;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinSet;
use tokio_stream::Stream;
use tokio_stream::StreamExt;

/// A message about the state of a hls download.
#[derive(Debug)]
pub enum DownloadHlsMessage {
    /// Downloaded the given media playlist
    DownloadedMediaPlaylist { media_playlist: Arc<MediaPlaylist> },

    /// Downloaded a single media segment
    DownloadedMediaSegment,

    /// Downloaded all media segments
    DownloadedAllMediaSegments,

    /// Concatenated a media segment
    ConcatenatedMediaSegment,

    /// Concatenated all media segments
    ConcatenatedAllMediaSegments,

    /// FFMPeg is now at the specified out time.
    FfmpegProgress { out_time: u64 },

    /// The download completed successfully.
    ///
    /// If this is not received, assume the download failed.
    Done,
}

/// Perform a hls download.
///
/// Returns a stream of events from the download.
/// An error is considered fatal, and will be the last message from this stream if it occurs.
pub fn download_hls<P>(
    client: reqwest::Client,
    url: &str,
    out_path: P,
) -> anyhow::Result<impl Stream<Item = Result<DownloadHlsMessage, anyhow::Error>>>
where
    P: AsRef<Path>,
{
    let _out_path = out_path.as_ref();
    let temp_out_path = nd_util::with_push_extension(&out_path, "part");
    let temp_dir_path = nd_util::with_push_extension(&out_path, "dir.part");
    let temp_dir_lock_file_path = temp_dir_path.join("lockfile");

    // Parse url.
    // Needed to resolve relative media segment urls.
    let mut url = Url::parse(url).context("invalid url")?;

    let stream = async_stream::try_stream! {
        // Create temp dir
        try_create_dir(&temp_dir_path)
            .await
            .with_context(|| format!("failed to create temp dir \"{}\"", temp_dir_path.display()))?;

        // We sometimes generate long names for temp files.
        // On Windows, this leads to issues with maximum path length.
        // We get a UNC path by canonicalizing, which extends the maximum path length and alleviates this issue.
        let temp_dir_path = tokio::fs::canonicalize(temp_dir_path)
            .await
            .context("failed to canonicalize temp dir")?;

        // Create lock file
        let lock_file = AsyncLockFile::create(temp_dir_lock_file_path)
            .await
            .context("failed to create temp dir lock file")?;

        // Lock temp dir.
        // This will prevent concurrent downloads to the same directory/file.
        // This lock should be held for the entire download.
        lock_file
            .try_lock()
            .await
            .context("failed to lock lock file")?;

        // TODO: Consider removing all entries in temp dir before donwloading.
        // However, we also key segment files uniquely per url.
        // The main risk is the server changing the stream from underneath us.
        // How would we detect that? Would it be better to just clear the dir each time?

        // TODO: Delete the temp dir on drop.
        // Since we "own" it at this point, we should clean it up if we fail.

        // Get the playlist
        let mut playlist_text = get_text(&client, url.as_str())
            .await
            .context("failed to download playlist")?;

        // Try to parse a master playlist
        match playlist_text.parse::<MasterPlaylist>() {
            Ok(master_playlist) => {
                // Select the best variant stream
                // TODO: Make user configurable
                // TODO: Report selected stream.
                let best_variant_stream = master_playlist
                    .variant_streams
                    .iter()
                    .max_by_key(|stream| stream.bandwidth)
                    .context("failed to select a variant stream")?;

                // Overwrite url with media playlist url, so relative media segments will work later.
                url = match best_variant_stream.uri.to_iri() {
                    Ok(absolute_uri) => Url::parse(absolute_uri.into())?,
                    Err(relative_uri) => url.join(relative_uri.into())?,
                };

                // Overwrite playlist text
                playlist_text = get_text(&client, url.as_str())
                    .await
                    .context("failed to download playlist")?;
            }
            Err(_error) => {
                // TODO: Proper master playlist detection
                // Since we failed, assume we have a media playlist and pass through.
            }
        }

        // Parse media playlist
        let media_playlist = playlist_text
            .parse::<MediaPlaylist>()
            .map(Arc::new)
            .context("invalid media playlist")?;

        yield DownloadHlsMessage::DownloadedMediaPlaylist {
            media_playlist: media_playlist.clone(),
        };

        // Download media segments, in parallel
        let mut media_segment_paths = Vec::with_capacity(media_playlist.media_segments.len());
        let mut join_set = JoinSet::new();
        let base_url = &url;
        for segment in media_playlist.media_segments.iter() {
            // We only support mgpeg2-ts streams for now,
            // since we know we can concat them.
            // TODO: Improve codec detection or add support for more codecs.
            if !segment.uri.path_str().ends_with(".ts") {
                Err(anyhow!("media segment path does not end with \".ts\""))?;
            }

            let client = client.clone();
            let url = segment.uri.to_iri();
            let url = match url {
                Ok(absolute_uri) => Url::parse(absolute_uri.into())?,
                Err(relative_uri) => base_url.join(relative_uri.into())?,
            };

            // Generated to be unique for segment uri.
            let file_name = url_to_file_name(url.as_str());

            let out_path = temp_dir_path.join(file_name);

            // Save out path for future concatenation
            media_segment_paths.push(out_path.clone());

            join_set.spawn(async move {
                if tokio::fs::try_exists(&out_path)
                    .await
                    .with_context(|| format!("failed to check if temp file at \"{}\" exists", out_path.display()))?
                {
                    return Ok(());
                }

                nd_util::download_to_path(&client, url.as_str(), &out_path)
                    .await
                    .with_context(|| format!("failed to download to media segment to \"{}\"", out_path.display()))
            });
        }

        // Process media segment download results
        while let Some(result) = join_set.join_next().await {
            let result = result
                .context("failed to join task")
                .and_then(std::convert::identity);
            result?;

            yield DownloadHlsMessage::DownloadedMediaSegment;
        }

        yield DownloadHlsMessage::DownloadedAllMediaSegments;

        // TODO: May be asyncified by stat-ing all component files,
        // calculating offsets,
        // then writing to different segments of the file concurrently.
        // This may even be done as part of the download process, avoiding the need of a second copy.
        // However, if the server does not send a content-length header, we must download everything to a seperate file.
        // A temp dir of some kind will always be required.
        // Copy to intermediate ts file
        let concat_file_name = url_to_file_name(url.as_str());
        let concat_file_path = temp_dir_path.join(&concat_file_name);

        {
            let mut dest_file = File::create(&concat_file_path).await?;
            for path in media_segment_paths {
                let mut src_file = File::open(path).await?;
                tokio::io::copy(&mut src_file, &mut dest_file).await?;

                yield DownloadHlsMessage::ConcatenatedMediaSegment;
            }

            dest_file.flush().await?;
            dest_file.sync_all().await?;

            yield DownloadHlsMessage::ConcatenatedAllMediaSegments;
        }

        // Remux concatenated file
        let mut concat_stream = tokio_ffmpeg_cli::Builder::new()
            .audio_codec("copy")
            .video_codec("copy")
            .input(concat_file_path)
            .output_format("mp4")
            .output(&temp_out_path)
            .overwrite(false)
            .spawn()
            .context("failed to spawn ffmpeg")?;

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
                    let out_time = match crate::parse_ffmpeg_time(&event.out_time) {
                        Ok(out_time) => out_time,
                        Err(e) => {
                            last_error = Err(e).context("failed to parse out time");
                            continue;
                        }
                    };

                    yield DownloadHlsMessage::FfmpegProgress {
                        out_time,
                    };
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

        if let Err(error) = last_error {
            if let Err(error) = tokio::fs::remove_file(&temp_out_path)
                .await
                .context("failed to remove temp file")
            {
                eprintln!("{error}");
            }

            Err(error)?;
        }

        tokio::fs::rename(&temp_out_path, &out_path)
            .await
            .context("failed to rename temp file")?;

        // Unlock and shutdown lock file
        // TODO: Can this be moved after the dir removal to prevent a race?
        lock_file.unlock().await.context("failed to unlock lock file")?;
        lock_file.shutdown().await?;
        drop(lock_file);

        tokio::fs::remove_dir_all(&temp_dir_path)
            .await
            .context("failed to remove temp dir")?;

        yield DownloadHlsMessage::Done;
    };

    Ok(stream)
}

async fn try_create_dir<P>(path: P) -> std::io::Result<()>
where
    P: AsRef<Path>,
{
    match tokio::fs::create_dir(path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(error);
        }
    }

    Ok(())
}

async fn get_text(client: &reqwest::Client, url: &str) -> Result<String, reqwest::Error> {
    client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await
}

fn url_to_file_name(url: &str) -> String {
    const MAX_FILE_NAME_LEN: usize = 248;

    let mut file_name = String::with_capacity(url.len());

    // File names on windows cannot exceed 248 bytes.
    // We add some room for path extension changes.
    // TODO: This may introduce clashes, use a hashing algo here.
    for c in url.chars() {
        if file_name.len() == MAX_FILE_NAME_LEN {
            break;
        }

        match c {
            '\\' | '/' | ':' | 'x' | '?' | '"' | '<' | '>' => {
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
