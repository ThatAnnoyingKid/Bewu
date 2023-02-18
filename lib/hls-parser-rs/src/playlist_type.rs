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
