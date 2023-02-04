use anyhow::ensure;
use anyhow::Context;

/// Parse an ffmpeg time, returning the time in seconds.
///
/// Format: `00:00:03.970612`, `HOURS:MM:SS.MICROSECONDS`
pub fn parse_ffmpeg_time(time: &str) -> anyhow::Result<u64> {
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
