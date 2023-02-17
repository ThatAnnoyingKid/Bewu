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

/// An error that may occur while parsing a tag
#[derive(Debug, thiserror::Error)]
pub enum ParseTagError {
    /// The tag was missing a colon
    #[error("missing a colon")]
    MissingColon,

    /// The tag was missing a comma
    #[error("missing a comma")]
    MissingComma,

    /// The tag was missing an equals
    #[error("missing equals")]
    MissingEquals,

    /// Invalid integer
    #[error("invalid integer")]
    ParseInt {
        /// The error
        #[source]
        error: std::num::ParseIntError,
    },

    /// Invalid float
    #[error("invalid float")]
    ParseFloat {
        /// The error
        #[source]
        error: std::num::ParseFloatError,
    },

    /// The tag is unknown
    #[error("unknown tag")]
    Unknown,

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

/// A tag
#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
enum Tag {
    /// The EXT-X-TARGETDURATION tag
    ExtXTargetDuration {
        /// The max length of each media segment
        duration: Duration,
    },

    /// The EXTINF tag
    ExtInf {
        /// The duration of the next segment
        duration: Duration,

        /// The title of the next media segment
        title: Option<Box<str>>,
    },

    /// The EXT-X-VERSION tag
    ExtXVersion {
        /// The version
        version: u8,
    },

    /// The EXT-X-MEDIA-SEQUENCE tag
    ExtXMediaSequence {
        /// The sequence number
        number: u64,
    },

    /// The EXT-X-KEY tag
    ExtXKey {},
}

impl std::str::FromStr for Tag {
    type Err = ParseTagError;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        if let Some(line) = line.strip_prefix(EXT_X_TARGET_DURATION_TAG) {
            let line = line.strip_prefix(':').ok_or(ParseTagError::MissingColon)?;
            let duration = line
                .parse()
                .map(Duration::from_secs)
                .map_err(|error| ParseTagError::ParseInt { error })?;
            Ok(Self::ExtXTargetDuration { duration })
        } else if let Some(line) = line.strip_prefix(EXT_INF_TAG) {
            let line = line.strip_prefix(':').ok_or(ParseTagError::MissingColon)?;
            let (duration, title) = line.split_once(',').ok_or(ParseTagError::MissingComma)?;
            let duration = duration
                .parse()
                .map(Duration::from_secs_f64)
                .map_err(|error| ParseTagError::ParseFloat { error })?;

            let title = match title.is_empty() {
                false => Some(title.into()),
                true => None,
            };

            Ok(Self::ExtInf { duration, title })
        } else if let Some(line) = line.strip_prefix(EXT_X_VERSION_TAG) {
            let line = line.strip_prefix(':').ok_or(ParseTagError::MissingColon)?;
            let version: u8 = line
                .parse()
                .map_err(|error| ParseTagError::ParseInt { error })?;
            Ok(Self::ExtXVersion { version })
        } else if let Some(line) = line.strip_prefix(EXT_X_MEDIA_SEQUENCE_TAG) {
            let line = line.strip_prefix(':').ok_or(ParseTagError::MissingColon)?;
            let number: u64 = line
                .parse()
                .map_err(|error| ParseTagError::ParseInt { error })?;

            Ok(Self::ExtXMediaSequence { number })
        } else if let Some(line) = line.strip_prefix(EXT_X_KEY_TAG) {
            let line = line.strip_prefix(':').ok_or(ParseTagError::MissingColon)?;
            let mut method = None;
            let mut uri = None;
            for pair in line.split(',') {
                let (name, value) = pair.split_once('=').ok_or(ParseTagError::MissingEquals)?;

                // TODO: Verify proper attributes are supplied with respect to current attribute state
                match name {
                    "METHOD" => {
                        if method.is_some() {
                            return Err(ParseTagError::DuplicateAttribute { name: name.into() });
                        }

                        method = Some(value);
                    }
                    "URI" => {
                        if uri.is_some() {
                            return Err(ParseTagError::DuplicateAttribute { name: name.into() });
                        }
                        uri = Some(value);
                    }
                    _ => {
                        return Err(ParseTagError::UnknownAttributeValuePair {
                            name: name.into(),
                            value: value.into(),
                        });
                    }
                }
            }

            Ok(Self::ExtXKey {})
        } else {
            Err(ParseTagError::Unknown)
        }
    }
}

#[cfg(test)]
mod test {
    //use super::*;
}
