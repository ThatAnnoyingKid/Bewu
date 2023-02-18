//! https://datatracker.ietf.org/doc/html/rfc8216

mod media_playlist;
mod tag;

pub use self::media_playlist::MediaPlaylist;
pub(crate) use self::tag::ParseTagError;
pub(crate) use self::tag::Tag;
pub use iri_string::types::UriReferenceStr;
pub use iri_string::types::UriReferenceString;
pub use iri_string::validate::Error as InvalidUriError;

const EXT_M3U_TAG: &str = "#EXTM3U";

const EXT_X_TARGET_DURATION_TAG: &str = "EXT-X-TARGETDURATION";
const EXT_INF_TAG: &str = "EXTINF";
const EXT_X_VERSION_TAG: &str = "EXT-X-VERSION";
const EXT_X_MEDIA_SEQUENCE_TAG: &str = "EXT-X-MEDIA-SEQUENCE";
const EXT_X_KEY_TAG: &str = "EXT-X-KEY";
const EXT_X_STREAM_INF_TAG: &str = "EXT-X-STREAM-INF";
const EXT_X_ALLOW_CACHE_TAG: &str = "EXT-X-ALLOW-CACHE";
const EXT_X_PLAYLIST_TYPE_TAG: &str = "EXT-X-PLAYLIST-TYPE";
const EXT_X_ENDLIST_TAG: &str = "EXT-X-ENDLIST";

/// Failed to parse a playlist type
#[derive(Debug)]
pub struct ParsePlaylistTypeError(pub Box<str>);

impl std::fmt::Display for ParsePlaylistTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\" is an invalid playlist type", self.0)
    }
}

impl std::error::Error for ParsePlaylistTypeError {}

/// The playlist type
#[derive(Debug)]
pub enum PlaylistType {
    /// The playlist cannot change
    Vod,

    /// The playlist is append only
    Event,
}

impl std::str::FromStr for PlaylistType {
    type Err = ParsePlaylistTypeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "VOD" => Ok(PlaylistType::Vod),
            "EVENT" => Ok(PlaylistType::Event),
            _ => Err(ParsePlaylistTypeError(input.into())),
        }
    }
}

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

    /// Duplicate tag
    #[error("duplicate tag \"{tag}\"")]
    DuplicateTag {
        /// The duplicate tag name
        tag: &'static str,
    },

    /// A URI was invalid
    #[error("invalid uri \"{line}\"")]
    InvalidUri {
        /// The line that failed to parse
        line: Box<str>,

        /// The uri parse error
        #[source]
        error: InvalidUriError,
    },

    /// Missing a tag
    #[error("missing tag \"{tag}\"")]
    MissingTag {
        /// The name of the missing tag
        tag: &'static str,
    },

    /// An error occured while parsing a tag
    #[error("tag parse error")]
    Tag {
        /// The inner error
        #[from]
        error: ParseTagError,
    },

    #[error("a tag was invalid in the given context")]
    InvalidTag,
}

/// A master playlist
#[derive(Debug)]
pub struct MasterPlaylist {
    /// A list of all variant streams
    pub variant_streams: Vec<VariantStream>,
}

impl std::str::FromStr for MasterPlaylist {
    type Err = Error;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let mut lines = input.lines();

        let start_tag = lines.next().ok_or(Error::UnexpectedEof)?;
        if start_tag != EXT_M3U_TAG {
            return Err(Error::InvalidStartTag {
                tag: start_tag.into(),
            });
        }

        let mut stream_info = None;
        let mut variant_streams = Vec::with_capacity(4);
        for line in lines {
            if line.is_empty() {
                continue;
            }

            if let Some(line) = line.strip_prefix('#') {
                if line.starts_with("EXT") {
                    let tag: Tag = line.parse::<Tag>()?;

                    match tag {
                        Tag::ExtXStreamInf {
                            bandwidth,
                            average_bandwidth,
                            codecs,
                            frame_rate,
                        } => {
                            if stream_info.is_some() {
                                return Err(Error::DuplicateTag {
                                    tag: EXT_X_STREAM_INF_TAG,
                                });
                            }

                            // TODO: Ensure this is immediately followed by a uri somehow.
                            stream_info = Some((bandwidth, average_bandwidth, codecs, frame_rate));
                        }
                        _ => {
                            return Err(Error::InvalidTag);
                        }
                    }
                }
            } else {
                let uri = UriReferenceStr::new(line).map_err(|error| Error::InvalidUri {
                    line: line.into(),
                    error,
                })?;
                let (bandwidth, average_bandwidth, codecs, frame_rate) =
                    stream_info.take().ok_or(Error::MissingTag {
                        tag: EXT_X_STREAM_INF_TAG,
                    })?;

                variant_streams.push(VariantStream {
                    uri: uri.into(),
                    bandwidth,
                    average_bandwidth,
                    codecs,
                    frame_rate,
                });
            }
        }

        Ok(Self { variant_streams })
    }
}

/// A variant stream
#[derive(Debug)]
pub struct VariantStream {
    /// The uri of the stream
    pub uri: UriReferenceString,

    /// The bandwidth of the stream
    pub bandwidth: u64,

    /// The average bandwidth
    pub average_bandwidth: Option<u64>,

    /// The codecs
    pub codecs: Option<Vec<Box<str>>>,

    /// The frame rate
    pub frame_rate: Option<f64>,
}

#[cfg(test)]
mod test {
    use super::*;

    const MASTER_PLAYLIST: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/master-playlist.m3u8"
    ));

    const REAL_MASTER_PLAYLIST_1: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/real-master-playlist-1.m3u8"
    ));

    #[test]
    fn parse_master_playlist() {
        let playlist: MasterPlaylist = MASTER_PLAYLIST.parse().expect("failed to parse");
        dbg!(&playlist);
    }

    #[test]
    fn parse_real_master_playlist_1() {
        let playlist: MasterPlaylist = REAL_MASTER_PLAYLIST_1.parse().expect("failed to parse");
        dbg!(&playlist);
    }
}
