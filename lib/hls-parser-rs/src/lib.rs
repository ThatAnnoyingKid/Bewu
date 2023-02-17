//! https://datatracker.ietf.org/doc/html/rfc8216

mod media_playlist;

pub use self::media_playlist::MediaPlaylist;
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

    /// An attribute value pair is missing an `=`
    #[error("attribute-value pair missing \"=\"")]
    AttributeValuePairMissingEquals,

    /// An attribute was duplicated
    #[error("duplicate attribute \"{name}\"")]
    DuplicateAttribute {
        /// The name of the duplicated attribute
        name: Box<str>,
    },

    /// An unknown attribute was supplied
    #[error("unknown attribute {name}={value}")]
    UnknownAttributeValuePair {
        /// The name of the attribute
        name: Box<str>,

        /// The value of the attribute
        value: Box<str>,
    },
}

fn try_parse_ext_target_duration_tag(line: &str) -> Option<Result<Duration, Error>> {
    line.strip_prefix(EXT_X_TARGET_DURATION_TAG).map(|line| {
        let line = line.strip_prefix(':').ok_or(Error::TagMissingColon {
            tag: EXT_X_TARGET_DURATION_TAG,
        })?;
        let duration = line
            .parse()
            .map(Duration::from_secs)
            .map_err(|error| Error::ExtXTargetDurationTagInvalidTime { error })?;

        Ok(duration)
    })
}

#[cfg(test)]
mod test {
    //use super::*;
}
