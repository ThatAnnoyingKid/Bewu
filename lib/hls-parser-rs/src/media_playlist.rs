use crate::Error;
use crate::Tag;
use crate::EXT_INF_TAG;
use crate::EXT_X_TARGET_DURATION_TAG;
use crate::EXT_X_VERSION_TAG;
use http::Uri;
use std::time::Duration;

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
                    let tag: Tag = line.parse::<Tag>()?;

                    match tag {
                        Tag::ExtXTargetDuration { duration } => {
                            if target_duration.is_some() {
                                return Err(Error::DuplicateTag {
                                    tag: EXT_X_TARGET_DURATION_TAG,
                                });
                            }

                            target_duration = Some(duration);
                        }
                        Tag::ExtInf { duration, title } => {
                            // Behavior of duped EXTINF tags is unspecified, use the latest one.
                            ext_inf_tag = Some((duration, title))
                        }
                        Tag::ExtXVersion { version: parsed } => {
                            if version.is_some() {
                                return Err(Error::DuplicateTag {
                                    tag: EXT_X_VERSION_TAG,
                                });
                            }

                            version = Some(parsed);
                        }
                        Tag::ExtXMediaSequence { number } => {
                            // TODO: Disallow setting after first segment?
                            // TODO: Disallow dupes?
                            media_sequence_number = Some(number);
                        }
                        Tag::ExtXKey {} => {
                            // TODO: Apply encryption data to media segments individually
                        }
                    }
                }
            } else {
                let uri: Uri = line.parse().map_err(|error| Error::InvalidUri { error })?;
                let (duration, title) = ext_inf_tag
                    .take()
                    .ok_or(Error::MissingTag { tag: EXT_INF_TAG })?;

                media_segments.push(MediaSegment {
                    duration,
                    title,
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

/// A media segment
#[derive(Debug, PartialEq, Eq)]
pub struct MediaSegment {
    /// The duration, in seconds
    pub duration: Duration,

    /// The title
    pub title: Option<Box<str>>,

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
                        title: None,
                        uri: Uri::from_static("http://media.example.com/first.ts"),
                    },
                    MediaSegment {
                        duration: Duration::from_secs_f64(9.009),
                        title: None,
                        uri: Uri::from_static("http://media.example.com/second.ts"),
                    },
                    MediaSegment {
                        duration: Duration::from_secs_f64(3.003),
                        title: None,
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
