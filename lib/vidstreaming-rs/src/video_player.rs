use crate::BASE_URL;
use cbc::cipher::KeyIvInit;
use cipher::block_padding::Pkcs7;
use cipher::BlockDecryptMut;
use cipher::BlockEncryptMut;
use once_cell::sync::Lazy;
use scraper::Html;
use scraper::Selector;
use std::collections::HashMap;
use url::Url;

type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

static LIST_SERVER_MORE_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("#list-server-more .linkserver[data-status=\"1\"]").unwrap());
static CRYPTO_DATA_VALUE_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("script[data-name=\"episode\"]").unwrap());
static REQUEST_KEY_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("body[class^='container-']").unwrap());
static REQUEST_IV_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("div[class*='container-']").unwrap());
static RESPONSE_KEY_SELECTOR: Lazy<Selector> =
    Lazy::new(|| Selector::parse("div[class*='videocontent-']").unwrap());

/// Error that may occur while parsing a [`VideoPlayer`]
#[derive(thiserror::Error, Debug)]
pub enum FromHtmlError {
    /// Missing crypto data value
    #[error("missing crypto data value")]
    MissingCryptoDataValue,

    /// Missing request key
    #[error("missing request key")]
    MissingRequestKey,

    /// Missing request iv
    #[error("missing request iv")]
    MissingRequestIv,

    /// Missing response key
    #[error("missing response key")]
    MissingResponseKey,

    /// Missing Link Server Url
    #[error("missing link server url")]
    MissingLinkServerUrl,

    /// Invalid URL
    #[error(transparent)]
    InvalidUrl(#[from] url::ParseError),
}

/// An error occured while using crypto to decrypt a crypto value
#[derive(thiserror::Error, Debug)]
pub enum DecryptCryptoDataValueError {
    /// Failed to decode a value from base64
    #[error(transparent)]
    Base64Decode(#[from] base64::DecodeError),

    /// Invalid Key or Iv length
    #[error(transparent)]
    InvalidKeyOrIvLength(#[from] cipher::InvalidLength),

    /// Padding error
    #[error(transparent)]
    Padding(#[from] cipher::block_padding::UnpadError),

    /// A value was not utf8
    #[error(transparent)]
    InvalidUtf8String(#[from] std::string::FromUtf8Error),
}

/// An error that may occur while encoding an id
#[derive(Debug, thiserror::Error)]
pub enum EncodeIdError {
    /// Invalid Key or Iv length
    #[error(transparent)]
    InvalidKeyOrIvLength(#[from] cipher::InvalidLength),
}

/// An error that may occur while generating a video data url
#[derive(Debug, thiserror::Error)]
pub enum GenerateVideoDataUrlError {
    /// An error occured while decrypting a crypto data value
    #[error("failed to decrypt `crypto-data-value`")]
    DecryptCryptoDataValue(#[from] DecryptCryptoDataValueError),

    /// Missing Video Id
    #[error("missing video id")]
    MissingVideoId,

    /// Missing url
    #[error("missing url")]
    MissingUrl,

    /// Missing url host
    #[error("missing url host")]
    MissingUrlHost,

    /// Failed to encode id
    #[error("failed to encode id")]
    EncodeId(#[from] EncodeIdError),
}

/// Video player for an episode
#[derive(Debug)]
pub struct VideoPlayer {
    /// crypto data value
    pub crypto_data_value: String,

    /// The request key
    pub request_key: String,

    /// The request iv
    pub request_iv: String,

    /// The response key
    pub response_key: String,

    /// Sources
    pub sources: Vec<Url>,
}

impl VideoPlayer {
    /// Try to make a [`VideoPlayer`] from html.
    pub(crate) fn from_html(html: &Html) -> Result<Self, FromHtmlError> {
        let crypto_data_value = html
            .select(&CRYPTO_DATA_VALUE_SELECTOR)
            .next()
            .and_then(|el| el.value().attr("data-value"))
            .ok_or(FromHtmlError::MissingCryptoDataValue)?
            .to_string();

        let request_key = html
            .select(&REQUEST_KEY_SELECTOR)
            .next()
            .and_then(|el| {
                el.value()
                    .classes
                    .iter()
                    .find_map(|class| class.strip_prefix("container-"))
            })
            .ok_or(FromHtmlError::MissingRequestKey)?
            .to_string();

        let request_iv = html
            .select(&REQUEST_IV_SELECTOR)
            .next()
            .and_then(|el| {
                el.value()
                    .classes
                    .iter()
                    .find_map(|class| class.strip_prefix("container-"))
            })
            .ok_or(FromHtmlError::MissingRequestIv)?
            .to_string();

        let response_key = html
            .select(&RESPONSE_KEY_SELECTOR)
            .next()
            .and_then(|el| {
                el.value()
                    .classes
                    .iter()
                    .find_map(|class| class.strip_prefix("videocontent-"))
            })
            .ok_or(FromHtmlError::MissingResponseKey)?
            .to_string();

        let sources = html
            .select(&LIST_SERVER_MORE_SELECTOR)
            .map(|el| {
                let url = el
                    .value()
                    .attr("data-video")
                    .ok_or(FromHtmlError::MissingLinkServerUrl)?;
                BASE_URL.join(url).map_err(FromHtmlError::InvalidUrl)
            })
            .collect::<Result<_, _>>()?;

        Ok(Self {
            crypto_data_value,
            request_key,
            request_iv,
            response_key,

            sources,
        })
    }

    /// Decrypt the `crypto_data_value` field.
    pub fn decrypt_crypto_data_value(&self) -> Result<String, DecryptCryptoDataValueError> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        let mut ciphertext = STANDARD.decode(&self.crypto_data_value)?;

        let cipher =
            Aes256CbcDec::new_from_slices(self.request_key.as_bytes(), self.request_iv.as_bytes())?;

        let decrypted_len = cipher.decrypt_padded_mut::<Pkcs7>(&mut ciphertext)?.len();
        ciphertext.truncate(decrypted_len);
        let decrypted = ciphertext;

        let decrypted = String::from_utf8(decrypted)?;

        Ok(decrypted)
    }

    /// Encode a video id
    pub fn encode_id(&self, id: &str) -> Result<String, EncodeIdError> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        let cipher =
            Aes256CbcEnc::new_from_slices(self.request_key.as_bytes(), self.request_iv.as_bytes())?;
        let encrypted_id = cipher.encrypt_padded_vec_mut::<Pkcs7>(id.as_bytes());
        let encrypted_base64_id = STANDARD.encode(encrypted_id);

        Ok(encrypted_base64_id)
    }

    /// Generate the url for the video data.
    pub fn generate_video_data_url(&self) -> Result<String, GenerateVideoDataUrlError> {
        let decrypted_crypto_data_value = self.decrypt_crypto_data_value()?;
        let (id, remaining_crypto_data_value) = decrypted_crypto_data_value
            .split_once('&')
            .ok_or(GenerateVideoDataUrlError::MissingVideoId)?;

        // TODO: The first url is usually the correct one,
        // but I'm not sure if that is always true.
        // Check here first if the generated url has the wrong host.
        let host = self
            .sources
            .first()
            .ok_or(GenerateVideoDataUrlError::MissingUrl)?
            .host_str()
            .ok_or(GenerateVideoDataUrlError::MissingUrlHost)?;
        let encoded_id = self.encode_id(id)?;

        // TODO: Ideally we would use a http::Uri here.
        // However, that type is extremly handicapped and almost unsuable.
        // Revist if that type ever provides a sane way to dynamically specify path and query parameters,
        // in a way that handles percent-encoding.
        let url = format!("https://{host}/encrypt-ajax.php?id={encoded_id}&{remaining_crypto_data_value}&alias={id}");

        Ok(url)
    }
}

/// An error that may occur while decrypting video data
#[derive(Debug, thiserror::Error)]
pub enum DecryptVideoDataError {
    /// Failed to decode a value from base64
    #[error(transparent)]
    Base64Decode(#[from] base64::DecodeError),

    /// Invalid Key or Iv length
    #[error(transparent)]
    InvalidKeyOrIvLength(#[from] cipher::InvalidLength),

    /// Padding error
    #[error(transparent)]
    Padding(#[from] cipher::block_padding::UnpadError),

    /// A value was not utf8
    #[error(transparent)]
    InvalidUtf8String(#[from] std::string::FromUtf8Error),

    /// Failed to parse video data
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// The encrypted video data
#[derive(Debug, serde::Deserialize)]
pub struct EncryptedVideoData {
    /// The encrypted video data
    pub data: String,
}

impl EncryptedVideoData {
    /// Decrypt this video data
    pub fn decrypt(&self, player: &VideoPlayer) -> Result<VideoData, DecryptVideoDataError> {
        use base64::engine::general_purpose::STANDARD;
        use base64::Engine;

        let mut ciphertext = STANDARD.decode(&self.data)?;

        let cipher = Aes256CbcDec::new_from_slices(
            player.response_key.as_bytes(),
            player.request_iv.as_bytes(),
        )?;

        let decrypted_len = cipher.decrypt_padded_mut::<Pkcs7>(&mut ciphertext)?.len();
        ciphertext.truncate(decrypted_len);
        let decrypted = ciphertext;

        let decrypted = String::from_utf8(decrypted)?;
        let video_data = serde_json::from_str(&decrypted)?;

        Ok(video_data)
    }
}

/// Video data
#[derive(Debug, serde::Deserialize)]
pub struct VideoData {
    /// The sources
    pub source: Vec<Source>,

    /// The original sources?
    pub source_bk: Vec<Source>,

    /// ?
    pub advertising: Vec<serde_json::Value>,

    /// The back-up url if the sources fail
    pub linkiframe: Url,

    /// Unknown KVs
    #[serde(flatten)]
    pub unknown: HashMap<String, serde_json::Value>,
}

impl VideoData {
    /// Get the best source.
    pub fn get_best_source(&self) -> Option<&Source> {
        let mut source_1080_p = None;
        let mut source_720_p = None;
        let mut source_480_p = None;
        let mut source_360_p = None;
        let mut source_hls = None;
        let mut source_auto_p = None;

        for source in self.source.iter() {
            match source.label.as_str() {
                "1080 P" => {
                    source_1080_p = Some(source);
                }
                "720 P" => {
                    source_720_p = Some(source);
                }
                "480 P" => {
                    source_480_p = Some(source);
                }
                "360 P" => {
                    source_360_p = Some(source);
                }
                "hls P" => {
                    source_hls = Some(source);
                }
                "auto P" => {
                    source_auto_p = Some(source);
                }
                _ => {}
            }
        }

        source_1080_p
            .or(source_720_p)
            .or(source_480_p)
            .or(source_360_p)
            .or(source_hls)
            .or(source_auto_p)
    }
}

/// Video source
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Source {
    /// Video url
    pub file: Url,

    /// Stream label
    pub label: String,

    /// Stream kind
    #[serde(rename = "type")]
    pub kind: String,

    /// Unknown KVs
    #[serde(flatten)]
    pub unknown: HashMap<String, serde_json::Value>,
}

impl Source {
    /// Returns true if this is an mp4
    pub fn is_mp4(&self) -> bool {
        // A source can lie sometimes, so do a basic check to make sure the url doesn't look like a m3u8
        matches!(self.kind.as_str(), "mp4") && !self.file.path().ends_with("m3u8")
    }

    /// Returns true if this is an hls stream
    pub fn is_hls(&self) -> bool {
        // A source can lie sometimes, so do a basic check see if the url looks like a m3u8
        matches!(self.kind.as_str(), "hls") || self.file.path().ends_with("m3u8")
    }
}

#[cfg(test)]
mod test {
    use super::*;

    const VIDEO_PLAYER_IRUMA: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/test_data/video_player/iruma.html"
    ));

    #[test]
    fn parse_iruma() {
        let html = Html::parse_document(VIDEO_PLAYER_IRUMA);
        let player = VideoPlayer::from_html(&html).expect("failed to parse");

        assert!(!player.sources.is_empty());
        dbg!(&player);
    }
}
