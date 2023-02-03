/// An error that may occur while parsing an [`AnimeType`]
#[derive(Debug, PartialEq, Clone, Hash)]
pub struct FromStrError(String);

impl std::fmt::Display for FromStrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid anime type {}", self.0)
    }
}

impl std::error::Error for FromStrError {}

/// Anime Type
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub enum AnimeType {
    /// Dubbed
    Dub,

    /// Subbed
    Sub,

    /// RAW
    Raw,
}

impl AnimeType {
    /// Get this as a str.
    ///
    /// String representations are 3 letters, all capitals.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sub => "SUB",
            Self::Dub => "DUB",
            Self::Raw => "RAW",
        }
    }
}

impl AnimeType {
    /// Returns `true` if it is `Dub`.
    pub fn is_dub(self) -> bool {
        matches!(self, Self::Dub)
    }

    /// Returns `true` if it is `Sub`.
    pub fn is_sub(self) -> bool {
        matches!(self, Self::Sub)
    }

    /// Returns `true` if it is `Raw`.
    pub fn is_raw(self) -> bool {
        matches!(self, Self::Raw)
    }
}

impl std::str::FromStr for AnimeType {
    type Err = FromStrError;

    fn from_str(data: &str) -> Result<Self, Self::Err> {
        match data {
            "DUB" => Ok(Self::Dub),
            "SUB" => Ok(Self::Sub),
            "RAW" => Ok(Self::Raw),
            _ => Err(FromStrError(data.to_string())),
        }
    }
}
