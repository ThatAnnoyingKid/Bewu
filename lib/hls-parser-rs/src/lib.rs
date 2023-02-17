pub use http::uri::InvalidUri as InvalidUriError;
pub use http::uri::Uri;

const EXT_X_TARGET_DURATION_TAG: &str = "EXT-X-TARGETDURATION";
const EXT_INF_TAG: &str = "EXTINF";

/// The library error type
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Unexpected EOF
    #[error("the file ended unexpectedly")]
    UnexpectedEof,

    #[error("invalid start tag \"{tag}\"")]
    InvalidStartTag {
        /// The unexpected tag
        tag: Box<str>,
    },

    #[error("unknown tag")]
    UnknownTag {
        /// The unknown tag
        tag: Box<str>,
    },

    /// Missing colon in `EXT-X-TARGETDURATION` tag
    #[error("tag \"{EXT_X_TARGET_DURATION_TAG}\" is missing a \":\"")]
    ExtXTargetDurationTagMissingColon,

    /// Invalid EXT_X_TARGET_DURATION_TAG time
    #[error("tag \"{EXT_X_TARGET_DURATION_TAG}\" provided an invalid time")]
    ExtXTargetDurationTagInvalidTime {
        #[source]
        error: std::num::ParseIntError,
    },

    /// Duplicate tag
    #[error("duplicate tag \"{tag}\"")]
    DuplicateTag {
        /// The duplicate tag name
        tag: &'static str,
    },

    /// Missing colon in `EXTINF` tag
    #[error("tag \"{EXT_INF_TAG}\" is missing a \":\"")]
    ExtInfTagMissingColon,

    /// Missing comma in `EXTINF` tag
    #[error("tag \"{EXT_INF_TAG}\" is missing a \",\"")]
    ExtInfTagMissingComma,

    /// Invalid EXTINF duration
    #[error("tag \"{EXT_INF_TAG}\" provided an invalid duration")]
    ExtInfTagInvalidDuration {
        #[source]
        error: std::num::ParseFloatError,
    },

    /// A URI was invalid
    #[error("invalid uri")]
    InvalidUri { error: InvalidUriError },

    /// Missing "EXTINF" tag
    #[error("missing tag \"{EXT_INF_TAG}\"")]
    MissingExtInfTag,
}

/// A media playlist
#[derive(Debug)]
pub struct MediaPlaylist {
    /// The media segments
    pub media_segments: Vec<MediaSegment>,
}

impl std::str::FromStr for MediaPlaylist {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Error> {
        let mut lines = input.lines();

        let start_tag = lines.next().ok_or(Error::UnexpectedEof)?;
        if start_tag != "#EXTM3U" {
            return Err(Error::InvalidStartTag {
                tag: start_tag.into(),
            });
        }

        let mut target_duration = None;
        let mut ext_inf_tag = None;
        let mut media_segments = Vec::with_capacity(16);
        for line in lines {
            if line.is_empty() {
                continue;
            }

            if let Some(line) = line.strip_prefix('#') {
                if line.starts_with("EXT") {
                    if let Some(line) = line.strip_prefix(EXT_X_TARGET_DURATION_TAG) {
                        let line = line
                            .strip_prefix(':')
                            .ok_or(Error::ExtXTargetDurationTagMissingColon)?;
                        let duration: u64 = line
                            .parse()
                            .map_err(|error| Error::ExtXTargetDurationTagInvalidTime { error })?;

                        if target_duration.is_some() {
                            return Err(Error::DuplicateTag {
                                tag: EXT_X_TARGET_DURATION_TAG,
                            });
                        }

                        target_duration = Some(duration);
                    } else if let Some(line) = line.strip_prefix(EXT_INF_TAG) {
                        let line = line.strip_prefix(':').ok_or(Error::ExtInfTagMissingColon)?;
                        let (duration, title) =
                            line.split_once(',').ok_or(Error::ExtInfTagMissingComma)?;
                        let duration: f64 = duration
                            .parse()
                            .map_err(|error| Error::ExtInfTagInvalidDuration { error })?;

                        // Behavior of duped EXTINF tags is unspecified, use the latest one.

                        ext_inf_tag = Some((duration, title))
                    } else {
                        return Err(Error::UnknownTag { tag: line.into() });
                    }
                }
            } else {
                let uri: Uri = line.parse().map_err(|error| Error::InvalidUri { error })?;
                let (duration, title) = ext_inf_tag.take().ok_or(Error::MissingExtInfTag)?;

                media_segments.push(MediaSegment {
                    duration,
                    title: title.into(),
                    uri,
                })
            }
        }

        Ok(Self { media_segments })
    }
}

/// A media segment
#[derive(Debug)]
pub struct MediaSegment {
    /// The duration, in seconds
    pub duration: f64,

    /// The title
    pub title: Box<str>,

    /// The uri
    pub uri: Uri,
}

#[cfg(test)]
mod test {
    use super::*;

    const SIMPLE_MEDIA_PLAYLIST: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/simple-media-playlist.m3u8"
    ));

    #[test]
    fn parse_simple_media_playlist() {
        let playlist: MediaPlaylist = SIMPLE_MEDIA_PLAYLIST.parse().expect("failed to parse");
        dbg!(&playlist);
    }
}
