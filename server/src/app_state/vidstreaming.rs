use crate::util::AbortJoinHandle;
use anyhow::anyhow;
use anyhow::ensure;
use anyhow::Context;
use pikadick_util::ArcAnyhowError;
use std::path::Path;
use std::sync::Arc;
use tokio_stream::Stream;
use tokio_stream::StreamExt;
use tracing::debug;
use tracing::error;
use tracing::trace;

#[derive(Debug)]
pub struct VidstreamingEpisode {
    pub path: Option<String>,
}

#[derive(Debug)]
pub enum VidstreamingTaskMessage {
    Close {
        tx: tokio::sync::oneshot::Sender<()>,
    },
    StartEpisodeDownload {
        anime_slug: Box<str>,
        episode_number: u32,

        tx: tokio::sync::oneshot::Sender<
            anyhow::Result<bewu_util::StateUpdateRx<CloneDownloadState>>,
        >,
    },
    GetEpisode {
        anime_slug: Box<str>,
        episode_number: u32,

        tx: tokio::sync::oneshot::Sender<anyhow::Result<VidstreamingEpisode>>,
    },
}

#[derive(Debug)]
pub struct VidstreamingTask {
    tx: tokio::sync::mpsc::Sender<VidstreamingTaskMessage>,
    handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl VidstreamingTask {
    pub fn new<P>(path: P) -> Self
    where
        P: AsRef<Path>,
    {
        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let handle = tokio::spawn(vidstreaming_task_impl(rx, path.as_ref().into()));

        Self {
            tx,
            handle: std::sync::Mutex::new(Some(handle)),
        }
    }

    pub async fn close(&self) -> anyhow::Result<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(VidstreamingTaskMessage::Close { tx }).await?;
        Ok(rx.await?)
    }

    pub async fn start_episode_download(
        &self,
        anime_slug: &str,
        episode_number: u32,
    ) -> anyhow::Result<impl Stream<Item = bewu_util::StateUpdateItem<CloneDownloadState>>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(VidstreamingTaskMessage::StartEpisodeDownload {
                anime_slug: anime_slug.into(),
                episode_number,
                tx,
            })
            .await?;

        Ok(rx.await??.into_stream())
    }

    pub async fn get_episode(
        &self,
        anime_slug: &str,
        episode_number: u32,
    ) -> anyhow::Result<VidstreamingEpisode> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx
            .send(VidstreamingTaskMessage::GetEpisode {
                anime_slug: anime_slug.into(),
                episode_number,
                tx,
            })
            .await?;
        rx.await?
    }

    pub async fn join(&self) -> anyhow::Result<()> {
        let handle = self
            .handle
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .context("missing handle")?;
        Ok(handle.await?)
    }

    pub async fn shutdown(&self) -> anyhow::Result<()> {
        let close_result = self.close().await;
        let join_result = self.join().await;
        join_result.or(close_result)
    }
}

async fn vidstreaming_task_impl(
    mut rx: tokio::sync::mpsc::Receiver<VidstreamingTaskMessage>,
    path: Arc<Path>,
) {
    let client = vidstreaming::Client::new();

    let mut download_task: Option<AbortJoinHandle<()>> = None;

    while let Some(message) = rx.recv().await {
        match message {
            VidstreamingTaskMessage::Close { tx } => {
                rx.close();
                let _ = tx.send(()).is_ok();
            }
            VidstreamingTaskMessage::StartEpisodeDownload {
                anime_slug,
                episode_number,
                tx,
            } => {
                let result = async {
                    let (tx, rx) = bewu_util::state_update_channel(128, CloneDownloadState::new());

                    if let Some(handle) = download_task.as_ref() {
                        ensure!(
                            handle.as_ref().is_finished(),
                            "another download is already in progress"
                        );
                        let handle = download_task.take().unwrap();
                        if let Err(e) = handle.into_inner().await.context("failed to join task") {
                            error!("{e:?}");
                        }
                    }

                    let handle = tokio::spawn(download_task_impl(
                        client.clone(),
                        anime_slug,
                        episode_number,
                        path.clone(),
                        tx,
                    ));
                    download_task = Some(AbortJoinHandle::new(handle));

                    Ok(rx)
                }
                .await;

                let _ = tx.send(result).is_ok();
            }
            VidstreamingTaskMessage::GetEpisode {
                anime_slug,
                episode_number,
                tx,
            } => {
                let result = async {
                    let file_name = get_episode_file_name(&anime_slug, episode_number);
                    let path = path.join(&file_name);

                    if tokio::fs::try_exists(&path).await? {
                        Ok(VidstreamingEpisode {
                            path: Some(file_name),
                        })
                    } else {
                        Ok(VidstreamingEpisode { path: None })
                    }
                }
                .await;

                let _ = tx.send(result).is_ok();
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum DownloadStateUpdate {
    Info { info: Arc<str> },
    Progress { progress: f32 },

    Error { error: ArcAnyhowError },
}

impl From<&str> for DownloadStateUpdate {
    fn from(info: &str) -> Self {
        Self::Info { info: info.into() }
    }
}

impl From<f32> for DownloadStateUpdate {
    fn from(progress: f32) -> Self {
        Self::Progress { progress }
    }
}

impl From<anyhow::Error> for DownloadStateUpdate {
    fn from(error: anyhow::Error) -> Self {
        Self::Error {
            error: ArcAnyhowError::new(error),
        }
    }
}

/// The state of a download
#[derive(Debug)]
pub struct DownloadState {
    pub info: Option<Arc<str>>,
    pub progress: f32,

    pub error: Option<ArcAnyhowError>,
}

impl DownloadState {
    fn new() -> Self {
        Self {
            info: None,
            progress: 0.0,

            error: None,
        }
    }

    fn apply_update(&mut self, update: &DownloadStateUpdate) {
        match update {
            DownloadStateUpdate::Error { error } => {
                // Only use the first error.
                if self.error.is_none() {
                    self.error = Some(error.clone());
                }
            }
            DownloadStateUpdate::Info { info } => {
                // Only keep the last info.
                self.info = Some(info.clone());
            }
            DownloadStateUpdate::Progress { progress } => {
                self.progress = *progress;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct CloneDownloadState {
    inner: Arc<std::sync::Mutex<DownloadState>>,
}

impl CloneDownloadState {
    fn new() -> Self {
        Self {
            inner: Arc::new(std::sync::Mutex::new(DownloadState::new())),
        }
    }

    pub fn get_inner(&self) -> std::sync::MutexGuard<'_, DownloadState> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl bewu_util::StateUpdateChannelState for CloneDownloadState {
    type Update = DownloadStateUpdate;

    fn apply_update(&self, update: &Self::Update) {
        self.get_inner().apply_update(update);
    }
}

fn get_episode_file_name(anime_slug: &str, episode_number: u32) -> String {
    format!("{anime_slug}-episode-{episode_number}.mp4")
}

async fn download_task_impl(
    client: vidstreaming::Client,
    anime_slug: Box<str>,
    episode_number: u32,
    path: Arc<Path>,
    download_state: bewu_util::StateUpdateTx<CloneDownloadState>,
) {
    // Guess vidstreaming url
    let url = format!("https://gogohd.net/videos/{anime_slug}-episode-{episode_number}");
    debug!("using vidstreaming url \"{url}\"");

    let file_name = get_episode_file_name(&anime_slug, episode_number);
    let out_path = path.join(file_name);
    match tokio::fs::try_exists(&out_path)
        .await
        .context("failed to check if episode exists")
    {
        Ok(true) => {
            download_state.send("already downloaded");
            return;
        }
        Ok(false) => {}
        Err(e) => {
            download_state.send(e);
        }
    }

    let episode = match client
        .get_episode(url.as_str())
        .await
        .context("failed to fetch episode")
    {
        Ok(episode) => {
            download_state.send("fetched episode");
            episode
        }
        Err(e) => {
            download_state.send(e);
            return;
        }
    };

    let video_player = match client
        .get_video_player(episode.video_player_url.as_str())
        .await
        .context("failed to fetch video player")
    {
        Ok(video_player) => {
            download_state.send("fetched video player");
            video_player
        }
        Err(e) => {
            download_state.send(e);
            return;
        }
    };

    let video_data = match client
        .get_video_player_video_data(&video_player)
        .await
        .context("failed to fetch video data")
    {
        Ok(video_data) => {
            download_state.send("fetched video data");
            video_data
        }
        Err(e) => {
            download_state.send(e);
            return;
        }
    };

    debug!("located {} sources", video_data.source.len());
    for source in video_data.source.iter() {
        debug!(
            "found source: (url={}, label={}, kind={})",
            source.file, source.label, source.kind
        );
    }

    debug!("located {} backup sources", video_data.source_bk.len());
    for source in video_data.source_bk.iter() {
        debug!(
            "found source: (url={}, label={}, kind={})",
            source.file, source.label, source.kind
        );
    }

    let best_source = match video_data
        .get_best_source()
        .context("failed to select a source")
    {
        Ok(source) => {
            download_state.send("selected video source");

            source
        }
        Err(e) => {
            download_state.send(e);
            return;
        }
    };

    debug!(
        "selected source: (url={}, label={}, kind={})",
        best_source.file, best_source.label, best_source.kind
    );

    let temp_path = nd_util::with_push_extension(&out_path, "part");

    let mut download_stream = match tokio_ffmpeg_cli::Builder::new()
        .audio_codec("copy")
        .video_codec("copy")
        .input(best_source.file.as_str())
        .output_format("mp4")
        .output(&temp_path)
        .overwrite(false)
        .spawn()
        .context("failed to spawn \"ffmpeg\"")
    {
        Ok(stream) => {
            download_state.send("started download");
            stream
        }
        Err(e) => {
            download_state.send(e);
            return;
        }
    };

    let mut exit_success = true;
    while let Some(event) = download_stream.next().await {
        let event = match event.context("download stream error") {
            Ok(event) => event,
            Err(e) => {
                download_state.send(e);
                continue;
            }
        };

        match event {
            tokio_ffmpeg_cli::Event::Progress(event) => {
                let out_time = match bewu_util::parse_ffmpeg_time(&event.out_time)
                    .context("failed to parse out time")
                {
                    Ok(out_time) => out_time,
                    Err(e) => {
                        download_state.send(e);
                        continue;
                    }
                };

                download_state.send(out_time as f32);
            }
            tokio_ffmpeg_cli::Event::ExitStatus(exit_status) => {
                if !exit_status.success() {
                    download_state.send(anyhow!("ffmpeg exit status was \"{exit_status:?}\""));
                    exit_success = false;
                }
            }
            tokio_ffmpeg_cli::Event::Unknown(line) => {
                trace!(line);
            }
        }
    }

    if exit_success {
        if let Err(e) = tokio::fs::rename(temp_path, out_path)
            .await
            .context("failed to rename")
        {
            download_state.send(e);
        }
    } else if let Err(e) = tokio::fs::remove_file(temp_path)
        .await
        .context("failed to remove temp file")
    {
        download_state.send(e);
    }
}
