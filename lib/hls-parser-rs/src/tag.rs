use crate::EXT_INF_TAG;
use crate::EXT_X_KEY_TAG;
use crate::EXT_X_MEDIA_SEQUENCE_TAG;
use crate::EXT_X_STREAM_INF_TAG;
use crate::EXT_X_TARGET_DURATION_TAG;
use crate::EXT_X_VERSION_TAG;
use std::time::Duration;

const BANDWIDTH_ATTR: &str = "BANDWIDTH";
const AVERAGE_BANDWIDTH_ATTR: &str = "AVERAGE-BANDWIDTH";
const CODECS_ATTR: &str = "CODECS";
const PROGRAM_ID_ATTR: &str = "PROGRAM-ID";
const RESOLUTION_ATTR: &str = "RESOLUTION";
const FRAME_RATE_ATTR: &str = "FRAME-RATE";

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
    Unknown { line: Box<str> },

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

    /// Missing an attribute
    #[error("missing attribute \"{name}\"")]
    MissingAttribute {
        /// The name of the attribute
        name: &'static str,
    },
}

/// A tag
#[allow(clippy::enum_variant_names)]
#[derive(Debug)]
pub(crate) enum Tag {
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

    /// The EXT-X-STREAM-INF tag
    ExtXStreamInf {
        /// The stream bandwidth
        bandwidth: u64,

        /// The average bandwidth
        average_bandwidth: Option<u64>,

        /// The frame rate
        frame_rate: Option<f64>,
    },
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
                // TODO: Verify K/V
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
        } else if let Some(line) = line.strip_prefix(EXT_X_STREAM_INF_TAG) {
            let line = line.strip_prefix(':').ok_or(ParseTagError::MissingColon)?;

            let mut bandwidth = None;
            let mut average_bandwidth = None;
            let mut frame_rate = None;

            let mut input = line;
            while let Some((pair, rest)) = input.split_once(',') {
                // TODO: Verify K/V
                let (name, value) = pair.split_once('=').ok_or(ParseTagError::MissingEquals)?;
                input = match name {
                    BANDWIDTH_ATTR => {
                        let value: u64 = value
                            .parse()
                            .map_err(|error| ParseTagError::ParseInt { error })?;
                        if bandwidth.is_some() {
                            return Err(ParseTagError::DuplicateAttribute { name: name.into() });
                        }
                        bandwidth = Some(value);
                        rest
                    }
                    AVERAGE_BANDWIDTH_ATTR => {
                        let value: u64 = value
                            .parse()
                            .map_err(|error| ParseTagError::ParseInt { error })?;
                        if average_bandwidth.is_some() {
                            return Err(ParseTagError::DuplicateAttribute { name: name.into() });
                        }
                        average_bandwidth = Some(value);
                        rest
                    }
                    CODECS_ATTR => {
                        // TODO: Parse Codec
                        // todo!("{value}");
                        rest
                    }
                    PROGRAM_ID_ATTR => {
                        // TODO: This was removed
                        // Consider adding if important
                        rest
                    }
                    RESOLUTION_ATTR => rest,
                    FRAME_RATE_ATTR => {
                        let value: f64 = value
                            .parse()
                            .map_err(|error| ParseTagError::ParseFloat { error })?;
                        frame_rate = Some(value);
                        rest
                    }
                    _ => {
                        return Err(ParseTagError::UnknownAttributeValuePair {
                            name: name.into(),
                            value: value.into(),
                        });
                    }
                }
            }

            let bandwidth = bandwidth.ok_or(ParseTagError::MissingAttribute {
                name: BANDWIDTH_ATTR,
            })?;

            Ok(Self::ExtXStreamInf {
                bandwidth,
                average_bandwidth,
                frame_rate,
            })
        } else {
            Err(ParseTagError::Unknown { line: line.into() })
        }
    }
}

/*
/// An error that may occur while parsing an attribute list
#[derive(Debug, thiserror::Error)]
enum ParseAttributeListError {
    /// Got an unexpected char
    #[error("unexpected char '{actual}', expected '{expected}'")]
    UnexpectedChar { expected: char, actual: char },

    /// Unexpected end of input
    #[error("unexpected end of input")]
    UnexpectedEnd,
}

#[derive(Debug)]
struct ParseAttributeListIter<'a> {
    input: &'a str,
    iter: std::iter::Peekable<std::str::CharIndices<'a>>,
}

impl<'a> ParseAttributeListIter<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            input,
            iter: input.char_indices().peekable(),
        }
    }
}

impl<'a> Iterator for ParseAttributeListIter<'a> {
    type Item = Result<(&'a str, &'a str), ParseAttributeListError>;

    fn next(&mut self) -> Option<Self::Item> {
        let (key_start_i, key_start_c) = self.iter.peek().copied()?;
        if !is_valid_attribute_key_char(key_start_c) {
            todo!();
        }
        self.iter.next();

        let mut key_end_i = key_start_i;
        while let Some((i, c)) = self.iter.peek() {
            if !is_valid_attribute_key_char(*c) {
                break;
            }
            key_end_i = *i + 1;
            self.iter.next();
        }

        match self
            .iter
            .next()
            .ok_or(ParseAttributeListError::UnexpectedEnd)
        {
            Ok((_, '=')) => {}
            Ok((_, c)) => {
                return Some(Err(ParseAttributeListError::UnexpectedChar {
                    expected: '=',
                    actual: c,
                }));
            }
            Err(e) => {
                return Some(Err(e));
            }
        };

        dbg!(self.iter.peek());

        todo!("{:#?}", &self.input[key_start_i..key_end_i]);

        None
    }
}

fn is_valid_attribute_key_char(c: char) -> bool {
    matches!(c,  'A'..='Z' | '0'..='9' | '-')
}
*/
