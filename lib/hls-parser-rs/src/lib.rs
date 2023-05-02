//! https://datatracker.ietf.org/doc/html/rfc8216
//! https://datatracker.ietf.org/doc/html/draft-pantos-hls-rfc8216bis

mod master_playlist;
mod media_playlist;
mod playlist_type;
mod tag;

pub use self::master_playlist::MasterPlaylist;
pub use self::master_playlist::VariantStream;
pub use self::media_playlist::MediaPlaylist;
pub use self::playlist_type::ParsePlaylistTypeError;
pub use self::playlist_type::PlaylistType;
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
const EXT_X_INDEPENDENT_SEGMENTS_TAG: &str = "EXT-X-INDEPENDENT-SEGMENTS";

/// An error that may occur while parsing a video range
#[derive(Debug)]
pub struct ParseVideoRangeError(Box<str>);

impl std::fmt::Display for ParseVideoRangeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "\"{}\" is not a valid VIDEO-RANGE", self.0)
    }
}

impl std::error::Error for ParseVideoRangeError {}

/// Video Ranges
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash, Default)]
pub enum VideoRange {
    /// SDR
    #[default]
    Sdr,

    /// HLG
    Hlg,

    /// PQ
    Pq,
}

impl std::str::FromStr for VideoRange {
    type Err = ParseVideoRangeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input {
            "SDR" => Ok(Self::Sdr),
            "HLQ" => Ok(Self::Hlg),
            "PQ" => Ok(Self::Pq),
            _ => Err(ParseVideoRangeError(input.into())),
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
