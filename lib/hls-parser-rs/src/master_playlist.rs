use crate::Error;
use crate::Tag;
use crate::UriReferenceStr;
use crate::UriReferenceString;
use crate::VideoRange;
use crate::EXT_M3U_TAG;
use crate::EXT_X_STREAM_INF_TAG;

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
                            resolution,
                            frame_rate,
                            video_range,
                        } => {
                            if stream_info.is_some() {
                                return Err(Error::DuplicateTag {
                                    tag: EXT_X_STREAM_INF_TAG,
                                });
                            }

                            // TODO: Ensure this is immediately followed by a uri somehow.
                            stream_info = Some((
                                bandwidth,
                                average_bandwidth,
                                codecs,
                                resolution,
                                frame_rate,
                                video_range,
                            ));
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
                let (bandwidth, average_bandwidth, codecs, resolution, frame_rate, video_range) =
                    stream_info.take().ok_or(Error::MissingTag {
                        tag: EXT_X_STREAM_INF_TAG,
                    })?;

                variant_streams.push(VariantStream {
                    uri: uri.into(),
                    bandwidth,
                    average_bandwidth,
                    codecs,
                    resolution,
                    frame_rate,
                    video_range,
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

    /// The resolution
    pub resolution: Option<(u64, u64)>,

    /// The frame rate
    pub frame_rate: Option<f64>,

    /// The video range
    pub video_range: Option<VideoRange>,
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
