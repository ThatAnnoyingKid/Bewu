use anyhow::Context;
use url::Url;

#[derive(argh::FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "info")]
#[argh(description = "get information about an episode")]
pub struct Options {
    #[argh(positional, description = "the episode page to get info for")]
    pub url: Url,

    #[argh(
        option,
        long = "related-episodes-limit",
        description = "the number of related episodes to list",
        default = "0"
    )]
    pub related_episodes_limit: usize,

    #[argh(
        switch,
        long = "video-player",
        description = "whether to fetch and print information for the video player"
    )]
    pub video_player: bool,

    #[argh(
        switch,
        long = "video-player-video-data",
        description = "whether to fetch and print information for the video player's video data"
    )]
    pub video_player_video_data: bool,
}

pub async fn exec(client: vidstreaming::Client, options: Options) -> anyhow::Result<()> {
    let episode = client
        .get_episode(options.url.as_str())
        .await
        .with_context(|| format!("failed to get episode at \"{}\"", options.url.as_str()))?;

    let video_player_info = if options.video_player || options.video_player_video_data {
        let video_player = client
            .get_video_player(episode.video_player_url.as_str())
            .await
            .with_context(|| {
                format!(
                    "failed to get video player for url \"{}\"",
                    episode.video_player_url.as_str()
                )
            })?;

        let video_player_video_data = if options.video_player_video_data {
            let video_player_video_data = client
                .get_video_player_video_data(&video_player)
                .await
                .with_context(|| {
                format!(
                    "failed to get video data for video player \"{}\"",
                    episode.video_player_url.as_str()
                )
            })?;
            Some(video_player_video_data)
        } else {
            None
        };

        Some((video_player, video_player_video_data))
    } else {
        None
    };

    println!("Name: {}", episode.name);
    if let Some(description) = episode.description.as_deref() {
        println!("Description: {description}");
    }
    println!("Video Player Url: {}", episode.video_player_url.as_str());
    if options.related_episodes_limit > 0 {
        println!("Related Episodes: ");
        for (i, episode) in episode
            .related_episodes
            .iter()
            .enumerate()
            .take(options.related_episodes_limit)
        {
            println!("  {}) {}", i + 1, episode.name);
            println!("    Url: {}", episode.url);
            println!("    Type: {}", episode.anime_type.as_str());
        }
    }
    if let Some((video_player, video_player_video_data)) = video_player_info {
        println!("Video Player: ");
        if options.video_player {
            println!("  Crypto Data Value: {}", video_player.crypto_data_value);
            println!("  Request Key: {}", video_player.request_key);
            println!("  Request Iv: {}", video_player.request_iv);
            println!("  Response Key: {}", video_player.response_key);
            println!("  Sources: ");
            for (i, source) in video_player.sources.iter().enumerate() {
                println!("    {}) {}", i + 1, source);
            }
        }

        if let Some(video_data) = video_player_video_data {
            println!("  Video Data:");
            println!("    Sources: ");
            for (i, source) in video_data.source.iter().enumerate() {
                println!("      {}) {}", i + 1, source.label);
                println!("        Url: {}", source.file);
                println!("        Kind: {}", source.kind);
            }
            println!("    Backup Sources: ");
            for (i, source) in video_data.source_bk.iter().enumerate() {
                println!("      {}) {}", i + 1, source.label);
                println!("        Url: {}", source.file);
                println!("        Kind: {}", source.kind);
            }
            println!("      IFrame Link: {}", video_data.linkiframe.as_str());
            if !video_data.track.tracks.is_empty() {
                println!("    Tracks: {:?}", video_data.track.tracks);
            }
            if !video_data.advertising.is_empty() {
                println!("    Advertising: {:?}", video_data.advertising);
            }
            if !video_data.unknown.is_empty() {
                println!("    Unknown: {:#?}", video_data.unknown);
            }
        }
    }

    Ok(())
}
