use anyhow::ensure;
use anyhow::Context;

/// Parse an ffmpeg time, returning the time in seconds.
///
/// Format: `00:00:03.970612`, `HOURS:MM:SS.MICROSECONDS`
pub fn parse_ffmpeg_time(time: &str) -> anyhow::Result<u64> {
    // TODO: Find out why negative time can occur
    let mut iter = time.trim_start_matches('-').split(':');

    let hours = iter.next().context("missing hours")?;
    let hours: u64 = hours
        .parse()
        .with_context(|| format!("failed to parse hours \"{hours}\""))?;
    let minutes: u64 = iter
        .next()
        .context("missing minutes")?
        .parse()
        .context("failed to parse minutes")?;

    let seconds = iter.next().context("missing seconds")?;
    let (seconds, microseconds) = seconds
        .split_once('.')
        .context("failed to separate seconds and microseconds")?;
    let _microseconds: u64 = microseconds.parse()?;
    let seconds: u64 = seconds.parse()?;

    ensure!(iter.next().is_none());

    Ok((hours * 60 * 60) + (minutes * 60) + seconds)
}
