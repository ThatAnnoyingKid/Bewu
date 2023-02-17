//! https://datatracker.ietf.org/doc/html/rfc8216

pub use http::uri::InvalidUri as InvalidUriError;
pub use http::uri::Uri;
use std::time::Duration;

const EXT_X_TARGET_DURATION_TAG: &str = "EXT-X-TARGETDURATION";
const EXT_INF_TAG: &str = "EXTINF";
const EXT_X_VERSION_TAG: &str = "EXT-X-VERSION";
const EXT_X_MEDIA_SEQUENCE_TAG: &str = "EXT-X-MEDIA-SEQUENCE";
const EXT_X_KEY_TAG: &str = "EXT-X-KEY";

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

    /// Missing colon for a tag
    #[error("tag \"{tag}\" is missing a \":\"")]
    TagMissingColon {
        /// The tag name
        tag: &'static str,
    },

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

    /// Missing a tag
    #[error("missing tag \"{tag}\"")]
    MissingTag {
        /// The name of the missing tag
        tag: &'static str,
    },

    /// Invalid EXT-X-VERSION version
    #[error("tag \"{EXT_X_VERSION_TAG}\" provided an invalid version")]
    ExtXVersionTagInvalidVersion {
        #[source]
        error: std::num::ParseIntError,
    },

    /// Invalid EXT-X-MEDIA-SEQUENCE number
    #[error("tag \"{EXT_X_MEDIA_SEQUENCE_TAG}\" provided an invalid number")]
    ExtXMediaSequenceTagInvalidSequence {
        #[source]
        error: std::num::ParseIntError,
    },
}

/// A media playlist
#[derive(Debug)]
pub struct MediaPlaylist {
    /// The target duration
    pub target_duration: Duration,

    /// The media segments
    pub media_segments: Vec<MediaSegment>,

    /// The version
    pub version: Option<u8>,

    /// The media sequence number of this first media segment.
    ///
    /// If this is `None`, it can be assumed to be 0.
    pub media_sequence_number: Option<u64>,
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
        let mut version = None;
        let mut media_sequence_number = None;

        let mut ext_inf_tag = None;
        let mut media_segments = Vec::with_capacity(16);
        for line in lines {
            if line.is_empty() {
                continue;
            }

            if let Some(line) = line.strip_prefix('#') {
                if line.starts_with("EXT") {
                    if let Some(duration) = try_parse_ext_target_duration_tag(line) {
                        let duration = duration?;

                        if target_duration.is_some() {
                            return Err(Error::DuplicateTag {
                                tag: EXT_X_TARGET_DURATION_TAG,
                            });
                        }

                        target_duration = Some(duration);
                    } else if let Some(line) = line.strip_prefix(EXT_INF_TAG) {
                        let line = line
                            .strip_prefix(':')
                            .ok_or(Error::TagMissingColon { tag: EXT_INF_TAG })?;
                        let (duration, title) =
                            line.split_once(',').ok_or(Error::ExtInfTagMissingComma)?;
                        let duration = duration
                            .parse()
                            .map(Duration::from_secs_f64)
                            .map_err(|error| Error::ExtInfTagInvalidDuration { error })?;

                        // Behavior of duped EXTINF tags is unspecified, use the latest one.

                        ext_inf_tag = Some((duration, title))
                    } else if let Some(line) = line.strip_prefix(EXT_X_VERSION_TAG) {
                        let line = line.strip_prefix(':').ok_or(Error::TagMissingColon {
                            tag: EXT_X_VERSION_TAG,
                        })?;
                        let parsed: u8 = line
                            .parse()
                            .map_err(|error| Error::ExtXVersionTagInvalidVersion { error })?;

                        if version.is_some() {
                            return Err(Error::DuplicateTag {
                                tag: EXT_X_VERSION_TAG,
                            });
                        }

                        version = Some(parsed);
                    } else if let Some(line) = line.strip_prefix(EXT_X_MEDIA_SEQUENCE_TAG) {
                        let line = line.strip_prefix(':').ok_or(Error::TagMissingColon {
                            tag: EXT_X_MEDIA_SEQUENCE_TAG,
                        })?;
                        let parsed: u64 = line.parse().map_err(|error| {
                            Error::ExtXMediaSequenceTagInvalidSequence { error }
                        })?;

                        // TODO: Disallow setting after first segment?
                        // TODO: Disallow dupes?
                        media_sequence_number = Some(parsed);
                    } else if let Some(line) = line.strip_prefix(EXT_X_KEY_TAG) {
                        let _line = line.strip_prefix(':').ok_or(Error::TagMissingColon {
                            tag: EXT_X_MEDIA_SEQUENCE_TAG,
                        })?;

                        // TODO: Parse attribute list
                    } else {
                        return Err(Error::UnknownTag { tag: line.into() });
                    }
                }
            } else {
                let uri: Uri = line.parse().map_err(|error| Error::InvalidUri { error })?;
                let (duration, title) = ext_inf_tag
                    .take()
                    .ok_or(Error::MissingTag { tag: EXT_INF_TAG })?;

                media_segments.push(MediaSegment {
                    duration,
                    title: title.into(),
                    uri,
                })
            }
        }

        let target_duration = target_duration.ok_or(Error::MissingTag {
            tag: EXT_X_TARGET_DURATION_TAG,
        })?;

        // TODO: Reject if media segment times are higher than target duration?

        Ok(Self {
            target_duration,
            media_segments,
            version,
            media_sequence_number,
        })
    }
}

fn try_parse_ext_target_duration_tag(line: &str) -> Option<Result<Duration, Error>> {
    line.strip_prefix(EXT_X_TARGET_DURATION_TAG).map(|line| {
        let line = line
            .strip_prefix(':')
            .ok_or(Error::ExtXTargetDurationTagMissingColon)?;
        let duration = line
            .parse()
            .map(Duration::from_secs)
            .map_err(|error| Error::ExtXTargetDurationTagInvalidTime { error })?;

        Ok(duration)
    })
}

/// A media segment
#[derive(Debug, PartialEq, Eq)]
pub struct MediaSegment {
    /// The duration, in seconds
    pub duration: Duration,

    /// The title
    pub title: Box<str>,

    /// The uri
    pub uri: Uri,
}

#[cfg(test)]
mod test {
    use super::*;

    /// This is provided by the spec.
    /// Note that it is also invalid,
    /// as it omits setting the EXT-X-VERSION tag to 3 while using floating point times for EXTINF.
    ///
    /// Since the spec disagrees with itself, we choose the more lenient option.
    const SIMPLE_MEDIA_PLAYLIST: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/simple-media-playlist.m3u8"
    ));

    const LIVE_MEDIA_PLAYLIST_USING_HTTPS: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/live-media-playlist-using-https.m3u8"
    ));

    const PLAYLIST_WITH_ENCRYPTED_MEDIA_SEGMENTS: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/playlist-with-encrypted-media-segments.m3u8"
    ));

    #[test]
    fn parse_simple_media_playlist() {
        let playlist: MediaPlaylist = SIMPLE_MEDIA_PLAYLIST.parse().expect("failed to parse");
        assert!(playlist.target_duration == Duration::from_secs(10));
        assert!(
            playlist.media_segments
                == [
                    MediaSegment {
                        duration: Duration::from_secs_f64(9.009),
                        title: "".into(),
                        uri: Uri::from_static("http://media.example.com/first.ts"),
                    },
                    MediaSegment {
                        duration: Duration::from_secs_f64(9.009),
                        title: "".into(),
                        uri: Uri::from_static("http://media.example.com/second.ts"),
                    },
                    MediaSegment {
                        duration: Duration::from_secs_f64(3.003),
                        title: "".into(),
                        uri: Uri::from_static("http://media.example.com/third.ts"),
                    }
                ]
        );
        assert!(playlist.version.is_none());

        dbg!(&playlist);
    }

    #[test]
    fn parse_live_media_playlist_using_https() {
        let playlist: MediaPlaylist = LIVE_MEDIA_PLAYLIST_USING_HTTPS
            .parse()
            .expect("failed to parse");
        assert!(playlist.version == Some(3));
        assert!(playlist.media_sequence_number == Some(2680));

        dbg!(&playlist);
    }

    #[test]
    fn parse_playlist_with_encrypted_media_segments() {
        let playlist: MediaPlaylist = PLAYLIST_WITH_ENCRYPTED_MEDIA_SEGMENTS
            .parse()
            .expect("failed to parse");
        assert!(playlist.version == Some(3));

        dbg!(&playlist);
    }
}
