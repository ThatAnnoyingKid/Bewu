use crate::ParsePlaylistTypeError;
use crate::PlaylistType;
use crate::EXT_INF_TAG;
use crate::EXT_X_ALLOW_CACHE_TAG;
use crate::EXT_X_ENDLIST_TAG;
use crate::EXT_X_KEY_TAG;
use crate::EXT_X_MEDIA_SEQUENCE_TAG;
use crate::EXT_X_PLAYLIST_TYPE_TAG;
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

    /// An unknown attribute was supplied
    #[error("unknown attribute \"{name}\"")]
    UnknownAttribute {
        /// The name of the attribute
        name: Box<str>,
    },

    /// Missing an attribute
    #[error("missing attribute \"{name}\"")]
    MissingAttribute {
        /// The name of the attribute
        name: &'static str,
    },

    /// Failed to parse an attribute list
    #[error("failed to parse attribute list")]
    AttributeListParse {
        #[from]
        error: AttributeListParseError,
    },

    /// Invalid playlist type
    #[error("invalid playlist type")]
    InvalidPlaylistType {
        #[from]
        error: ParsePlaylistTypeError,
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

        /// The codecs
        codecs: Option<Vec<Box<str>>>,

        /// The frame rate
        frame_rate: Option<f64>,
    },

    /// The EXT-X-ALLOW_CACHE tag
    ExtXAllowCache {},

    /// The EXT-X-PLAYLIST-TYPE tag
    ExtXPlaylistType { playlist_type: PlaylistType },

    /// The EXT-X-ENDLIST tag
    ExtXEndList,
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
            let mut codecs = None;
            let mut frame_rate = None;

            let mut parser = AttributeListParser::new(line);
            loop {
                let name = parser.parse_name()?;
                parser.parse_equals()?;

                match name {
                    BANDWIDTH_ATTR => {
                        let value: u64 = parser.parse_decimal_integer()?;

                        if bandwidth.is_some() {
                            return Err(ParseTagError::DuplicateAttribute { name: name.into() });
                        }
                        bandwidth = Some(value);
                    }
                    AVERAGE_BANDWIDTH_ATTR => {
                        let value: u64 = parser.parse_decimal_integer()?;

                        if average_bandwidth.is_some() {
                            return Err(ParseTagError::DuplicateAttribute { name: name.into() });
                        }
                        average_bandwidth = Some(value);
                    }
                    CODECS_ATTR => {
                        let value = parser.parse_quoted_string()?;

                        if codecs.is_some() {
                            return Err(ParseTagError::DuplicateAttribute { name: name.into() });
                        }
                        codecs = Some(
                            value
                                .split(',')
                                .map(|s| s.into())
                                .collect::<Vec<Box<str>>>(),
                        );
                    }
                    PROGRAM_ID_ATTR => {
                        let _value = parser.parse_decimal_integer()?;
                        // TODO: This was removed from the spec
                        // Consider adding if important
                    }
                    RESOLUTION_ATTR => {
                        let _value = parser.parse_decimal_resolution()?;
                    }
                    FRAME_RATE_ATTR => {
                        let value = parser.parse_decimal_floating_point()?;

                        if frame_rate.is_some() {
                            return Err(ParseTagError::DuplicateAttribute { name: name.into() });
                        }
                        frame_rate = Some(value);
                    }
                    _ => {
                        return Err(ParseTagError::UnknownAttribute { name: name.into() });
                    }
                }
                match parser.parse_comma() {
                    Ok(()) => {}
                    Err(AttributeListParseError::UnexpectedEnd) => {
                        break;
                    }
                    Err(e) => {
                        return Err(ParseTagError::from(e));
                    }
                }
            }

            let bandwidth = bandwidth.ok_or(ParseTagError::MissingAttribute {
                name: BANDWIDTH_ATTR,
            })?;

            Ok(Self::ExtXStreamInf {
                bandwidth,
                average_bandwidth,
                codecs,
                frame_rate,
            })
        } else if let Some(_line) = line.strip_prefix(EXT_X_ALLOW_CACHE_TAG) {
            // TODO: This was removed in the spec
            // Add back if needed
            Ok(Self::ExtXAllowCache {})
        } else if let Some(line) = line.strip_prefix(EXT_X_PLAYLIST_TYPE_TAG) {
            let line = line.strip_prefix(':').ok_or(ParseTagError::MissingColon)?;
            let playlist_type: PlaylistType = line.parse()?;

            Ok(Self::ExtXPlaylistType { playlist_type })
        } else if let Some(_line) = line.strip_prefix(EXT_X_ENDLIST_TAG) {
            Ok(Self::ExtXEndList)
        } else {
            Err(ParseTagError::Unknown { line: line.into() })
        }
    }
}

/// An error that may occur while parsing an attribute list
#[derive(Debug, thiserror::Error)]
pub enum AttributeListParseError {
    /// Got an unexpected char
    #[error("unexpected char '{actual}', expected {expected}")]
    UnexpectedChar {
        expected: &'static str,
        actual: char,
    },

    /// Unexpected end of input
    #[error("unexpected end of input")]
    UnexpectedEnd,

    /// Invalid decimal integer
    #[error("invalid decimal integer")]
    InvalidDecimalInteger {
        #[source]
        error: std::num::ParseIntError,
    },

    /// Invalid decimal floating point
    #[error("invalid decimal floating point")]
    InvalidDecimalFloatingPoint {
        #[source]
        error: std::num::ParseFloatError,
    },
}

#[derive(Debug)]
struct AttributeListParser<'a> {
    input: &'a str,
    iter: std::iter::Peekable<std::str::CharIndices<'a>>,
}

impl<'a> AttributeListParser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            input,
            iter: input.char_indices().peekable(),
        }
    }

    /// Parse the name in a name=value pair
    fn parse_name(&mut self) -> Result<&'a str, AttributeListParseError> {
        let (start_i, start_c) = self
            .iter
            .peek()
            .copied()
            .ok_or(AttributeListParseError::UnexpectedEnd)?;
        if !is_valid_attribute_key_char(start_c) {
            return Err(AttributeListParseError::UnexpectedChar {
                expected: "an attribute name character",
                actual: start_c,
            });
        }
        self.iter.next();

        let mut end_i = start_i + 1;
        while let Some((i, c)) = self.iter.peek() {
            if !is_valid_attribute_key_char(*c) {
                break;
            }
            end_i = *i + 1;
            self.iter.next();
        }

        Ok(&self.input[start_i..end_i])
    }

    fn parse_equals(&mut self) -> Result<(), AttributeListParseError> {
        let (_start_i, start_c) = self
            .iter
            .peek()
            .copied()
            .ok_or(AttributeListParseError::UnexpectedEnd)?;
        if start_c != '=' {
            return Err(AttributeListParseError::UnexpectedChar {
                expected: "'='",
                actual: start_c,
            });
        }
        self.iter.next();

        Ok(())
    }

    fn parse_comma(&mut self) -> Result<(), AttributeListParseError> {
        let (_start_i, start_c) = self
            .iter
            .peek()
            .copied()
            .ok_or(AttributeListParseError::UnexpectedEnd)?;
        if start_c != ',' {
            return Err(AttributeListParseError::UnexpectedChar {
                expected: "','",
                actual: start_c,
            });
        }
        self.iter.next();

        Ok(())
    }

    fn parse_double_quote(&mut self) -> Result<(), AttributeListParseError> {
        let (_start_i, start_c) = self
            .iter
            .peek()
            .copied()
            .ok_or(AttributeListParseError::UnexpectedEnd)?;
        if start_c != '"' {
            return Err(AttributeListParseError::UnexpectedChar {
                expected: "'\"'",
                actual: start_c,
            });
        }
        self.iter.next();

        Ok(())
    }

    fn parse_x(&mut self) -> Result<(), AttributeListParseError> {
        let (_start_i, start_c) = self
            .iter
            .peek()
            .copied()
            .ok_or(AttributeListParseError::UnexpectedEnd)?;
        if start_c != 'x' {
            return Err(AttributeListParseError::UnexpectedChar {
                expected: "'x'",
                actual: start_c,
            });
        }
        self.iter.next();

        Ok(())
    }

    fn parse_decimal_integer(&mut self) -> Result<u64, AttributeListParseError> {
        let (start_i, start_c) = self
            .iter
            .peek()
            .copied()
            .ok_or(AttributeListParseError::UnexpectedEnd)?;
        if !start_c.is_ascii_digit() {
            return Err(AttributeListParseError::UnexpectedChar {
                expected: "a digit",
                actual: start_c,
            });
        }
        self.iter.next();

        let mut end_i = start_i + 1;
        while let Some((i, c)) = self.iter.peek() {
            if !c.is_ascii_digit() {
                break;
            }
            end_i = *i + 1;
            self.iter.next();
        }

        self.input[start_i..end_i]
            .parse()
            .map_err(|error| AttributeListParseError::InvalidDecimalInteger { error })
    }

    fn parse_quoted_string(&mut self) -> Result<&'a str, AttributeListParseError> {
        let (start_i, start_c) = self
            .iter
            .peek()
            .copied()
            .ok_or(AttributeListParseError::UnexpectedEnd)?;
        if start_c != '"' {
            return Err(AttributeListParseError::UnexpectedChar {
                expected: "a '\"'",
                actual: start_c,
            });
        }
        self.iter.next();

        let mut end_i = start_i + 1;
        while let Some((i, c)) = self.iter.peek() {
            if matches!(c, '\r' | '\n' | '"') {
                break;
            }
            end_i = *i + 1;
            self.iter.next();
        }
        self.parse_double_quote()?;

        Ok(&self.input[start_i + 1..end_i])
    }

    fn parse_decimal_resolution(&mut self) -> Result<(u64, u64), AttributeListParseError> {
        let w = self.parse_decimal_integer()?;
        self.parse_x()?;
        let h = self.parse_decimal_integer()?;
        Ok((w, h))
    }

    fn parse_decimal_floating_point(&mut self) -> Result<f64, AttributeListParseError> {
        let (start_i, start_c) = self
            .iter
            .peek()
            .copied()
            .ok_or(AttributeListParseError::UnexpectedEnd)?;
        if !start_c.is_ascii_digit() {
            return Err(AttributeListParseError::UnexpectedChar {
                expected: "a digit",
                actual: start_c,
            });
        }
        self.iter.next();

        let mut end_i = start_i + 1;
        while let Some((i, c)) = self.iter.peek() {
            if !c.is_ascii_digit() && *c != '.' {
                break;
            }
            end_i = *i + 1;
            self.iter.next();
        }

        self.input[start_i..end_i]
            .parse()
            .map_err(|error| AttributeListParseError::InvalidDecimalFloatingPoint { error })
    }
}

fn is_valid_attribute_key_char(c: char) -> bool {
    matches!(c,  'A'..='Z' | '0'..='9' | '-')
}
