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
}

pub async fn exec(client: vidstreaming::Client, options: Options) -> anyhow::Result<()> {
    let episode = client
        .get_episode(options.url.as_str())
        .await
        .with_context(|| format!("failed to get episode at \"{}\"", options.url.as_str()))?;

    println!("Name: {}", episode.name);
    if let Some(description) = episode.description.as_deref() {
        println!("Description: {}", description);
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

    /*
    let video_player = client
        .vidstreaming_client
        .get_video_player(episode.video_player_url.as_str())
        .await
        .context("failed to get video player")?;
    println!("Sources: ");
    for (i, source) in video_player.sources.iter().enumerate() {
        println!("{}) {}", i + 1, source);
    }
    println!();
    */

    Ok(())
}
