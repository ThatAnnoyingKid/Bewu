//! https://datatracker.ietf.org/doc/html/rfc8216

mod media_playlist;
mod tag;

pub use self::media_playlist::MediaPlaylist;
pub(crate) use self::tag::ParseTagError;
pub(crate) use self::tag::Tag;
pub use http::uri::InvalidUri as InvalidUriError;
pub use http::uri::Uri;

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

    /// Duplicate tag
    #[error("duplicate tag \"{tag}\"")]
    DuplicateTag {
        /// The duplicate tag name
        tag: &'static str,
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

    /// An error occured while parsing a tag
    #[error("tag parse error")]
    Tag {
        /// The inner error
        #[from]
        error: ParseTagError,
    },
}

#[cfg(test)]
mod test {
    //use super::*;
}
