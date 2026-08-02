//! Last.fm API client.

use std::collections::BTreeMap;
use std::sync::Arc;

use md5::{Digest, Md5};
use reqwest::Client;
use scraper::{Html, Selector};
use serde::Deserialize;

use super::models::{LastFmArtist, LastFmSession};

const LASTFM_API_URL: &str = "https://ws.audioscrobbler.com/2.0/";

/// Last.fm API client.
#[derive(Debug, Clone)]
pub struct LastFmClient {
    inner: LastFmClientInner,
}

#[derive(Debug, Clone)]
enum LastFmClientInner {
    /// Live Last.fm client with configured credentials.
    Live(Arc<Inner>),
    /// Last.fm integration is disabled because credentials are absent.
    Disabled,
}

#[derive(Debug)]
struct Inner {
    client: Client,
    api_key: String,
    api_secret: String,
}

/// Error type for Last.fm operations.
#[derive(Debug, thiserror::Error)]
pub enum LastFmError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("API error {code}: {message}")]
    Api { code: i32, message: String },
    #[error("scrobble ignored by Last.fm (code {code}): {message}")]
    Filtered { code: i64, message: String },
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    #[error("No session key for user")]
    NoSessionKey,
    #[error("Configuration error: {0}")]
    Config(String),
}

/// Result type for Last.fm operations.
pub type Result<T> = std::result::Result<T, LastFmError>;

impl LastFmClient {
    /// Create a new Last.fm client.
    ///
    pub fn new(api_key: String, api_secret: String) -> Result<Self> {
        if api_key.is_empty() || api_secret.is_empty() {
            return Ok(Self {
                inner: LastFmClientInner::Disabled,
            });
        }

        let client = Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64; rv:109.0) Gecko/20100101 Firefox/115.0")
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(LastFmError::Network)?;

        Ok(Self {
            inner: LastFmClientInner::Live(Arc::new(Inner {
                client,
                api_key,
                api_secret,
            })),
        })
    }

    /// Check if Last.fm is configured.
    #[must_use]
    pub const fn is_configured(&self) -> bool {
        matches!(self.inner, LastFmClientInner::Live(_))
    }

    /// Get the API key.
    ///
    /// # Errors
    /// Returns an error when Last.fm credentials are not configured.
    pub fn api_key(&self) -> Result<&str> {
        Ok(&self.inner()?.api_key)
    }

    fn inner(&self) -> Result<&Inner> {
        match self {
            Self {
                inner: LastFmClientInner::Live(inner),
            } => Ok(inner),
            Self {
                inner: LastFmClientInner::Disabled,
            } => Err(LastFmError::Config(
                "LASTFM_API_KEY and LASTFM_API_SECRET are required".to_string(),
            )),
        }
    }

    /// Sign API parameters according to Last.fm rules.
    /// The signature is: `md5(sorted_param_names_concatenated_with_values` + secret)
    fn sign_params(&self, params: &BTreeMap<String, String>) -> Result<String> {
        let mut signature_input = String::new();

        for (key, value) in params {
            signature_input.push_str(key);
            signature_input.push_str(value);
        }

        signature_input.push_str(&self.inner()?.api_secret);

        let mut hasher = Md5::new();
        hasher.update(signature_input.as_bytes());
        Ok(hex::encode(hasher.finalize()))
    }

    /// Build signed parameters for an API call.
    fn build_params(
        &self,
        method: &str,
        session_key: Option<&str>,
        extra: BTreeMap<String, String>,
    ) -> Result<BTreeMap<String, String>> {
        let mut params = BTreeMap::new();
        params.insert("method".to_string(), method.to_string());
        params.insert("api_key".to_string(), self.inner()?.api_key.clone());

        // Add extra params
        for (key, value) in extra {
            params.insert(key, value);
        }

        // Add session key if provided
        if let Some(sk) = session_key {
            params.insert("sk".to_string(), sk.to_string());
        }

        // Generate and add signature
        let signature = self.sign_params(&params)?;
        params.insert("api_sig".to_string(), signature);

        // Format must be added after signature
        params.insert("format".to_string(), "json".to_string());

        Ok(params)
    }

    /// Get a Last.fm session from a token.
    ///
    /// # Errors
    /// Returns an error when the HTTP request fails, Last.fm returns an API error,
    /// or the response body cannot be parsed.
    pub async fn get_session(&self, token: &str) -> Result<LastFmSession> {
        // Response struct defined locally
        #[derive(Deserialize)]
        struct SessionResponse {
            session: LastFmSession,
        }

        let mut extra = BTreeMap::new();
        extra.insert("token".to_string(), token.to_string());

        let params = self.build_params("auth.getSession", None, extra)?;

        let response = self
            .inner()?
            .client
            .get(LASTFM_API_URL)
            .query(&params)
            .send()
            .await?;

        let status = response.status();
        let body: String = response.text().await?;

        if let Ok(error) = serde_json::from_str::<LastFmApiError>(&body) {
            return Err(LastFmError::Api {
                code: error.error,
                message: error.message,
            });
        }

        if !status.is_success() {
            return Err(LastFmError::Api {
                code: i32::from(status.as_u16()),
                message: body,
            });
        }

        let parsed: SessionResponse = serde_json::from_str(&body)
            .map_err(|e| LastFmError::InvalidResponse(format!("Failed to parse: {e}")))?;

        Ok(parsed.session)
    }

    /// Submit a scrobble to Last.fm.
    ///
    /// # Errors
    /// Returns an error when the HTTP request fails or Last.fm returns an API error response.
    pub async fn scrobble(
        &self,
        session_key: &str,
        artist: &str,
        track: &str,
        album: Option<&str>,
        timestamp: i64,
    ) -> Result<()> {
        let mut extra = BTreeMap::new();
        extra.insert("artist".to_string(), artist.to_string());
        extra.insert("track".to_string(), track.to_string());
        extra.insert("timestamp".to_string(), timestamp.to_string());

        if let Some(album_name) = album {
            extra.insert("album".to_string(), album_name.to_string());
        }

        let params = self.build_params("track.scrobble", Some(session_key), extra)?;

        let response = self
            .inner()?
            .client
            .post(LASTFM_API_URL)
            .form(&params)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;

        check_lfm_error(status, &body)?;
        check_scrobble_accepted(&body)
    }

    /// Update now playing status on Last.fm.
    ///
    /// # Errors
    /// Returns an error when the HTTP request fails or Last.fm returns an API error response.
    pub async fn update_now_playing(
        &self,
        session_key: &str,
        artist: &str,
        track: &str,
        album: Option<&str>,
        duration: Option<i32>,
    ) -> Result<()> {
        let mut extra = BTreeMap::new();
        extra.insert("artist".to_string(), artist.to_string());
        extra.insert("track".to_string(), track.to_string());

        if let Some(album_name) = album {
            extra.insert("album".to_string(), album_name.to_string());
        }

        if let Some(dur) = duration {
            extra.insert("duration".to_string(), dur.to_string());
        }

        let params = self.build_params("track.updateNowPlaying", Some(session_key), extra)?;

        let response = self
            .inner()?
            .client
            .post(LASTFM_API_URL)
            .form(&params)
            .send()
            .await?;

        let status = response.status();
        let body = response.text().await?;
        check_lfm_error(status, &body)
    }

    /// Get artist information from Last.fm.
    ///
    /// # Errors
    /// Returns an error when the HTTP request fails, Last.fm returns an API error,
    /// or the response body cannot be parsed.
    pub async fn get_artist_info(&self, artist_name: &str) -> Result<Option<LastFmArtist>> {
        // Response struct defined at the top of function to avoid items_after_statements warning
        #[derive(Deserialize)]
        struct ArtistResponse {
            artist: Option<LastFmArtist>,
        }

        // For public data, no session key needed
        // Important: api_sig is NOT required for artist.getInfo unless authenticated
        // Including it with invalid logic causes "Invalid method signature" (error 13)
        // We simply build params manually to avoid signing logic
        let mut params = BTreeMap::new();
        params.insert("method".to_string(), "artist.getInfo".to_string());
        params.insert("api_key".to_string(), self.inner()?.api_key.clone());
        params.insert("format".to_string(), "json".to_string());
        params.insert("artist".to_string(), artist_name.to_string());
        params.insert("autocorrect".to_string(), "1".to_string());

        let response = self
            .inner()?
            .client
            .get(LASTFM_API_URL)
            .query(&params)
            .send()
            .await?;

        let status = response.status();
        let body: String = response.text().await?;

        if let Ok(error) = serde_json::from_str::<LastFmApiError>(&body) {
            return Err(LastFmError::Api {
                code: error.error,
                message: error.message,
            });
        }

        if !status.is_success() {
            return Err(LastFmError::Api {
                code: i32::from(status.as_u16()),
                message: body,
            });
        }

        // Parse the response
        let parsed: ArtistResponse = serde_json::from_str(&body)
            .map_err(|e| LastFmError::InvalidResponse(format!("Failed to parse: {e}")))?;

        Ok(parsed.artist)
    }

    /// Fetch the artist image from their Last.fm page by scraping the og:image meta tag.
    ///
    /// # Errors
    /// Returns an error when fetching or parsing the artist page fails.
    pub async fn fetch_artist_image_from_page(&self, url: &str) -> Result<Option<String>> {
        let response = self.inner()?.client.get(url).send().await?;
        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(LastFmError::Api {
                code: i32::from(status.as_u16()),
                message: format!("Failed to fetch artist page: {body}"),
            });
        }

        let document = Html::parse_document(&body);
        let selector = Selector::parse("meta[property=\"og:image\"]")
            .map_err(|e| LastFmError::InvalidResponse(format!("Invalid selector: {e:?}")))?;

        let image_url = if let Some(element) = document.select(&selector).next()
            && let Some(content) = element.value().attr("content")
            && !content.is_empty()
            && !content.contains("2a96cbd8b46e442fc41c2b86b821562f")
        {
            Some(content.to_string())
        } else {
            None
        };

        Ok(image_url)
    }
}

/// Check an lfm response for API errors, per the Scrobbling 2.0 spec.
///
/// The HTTP status code alone does not indicate success or failure; the
/// response body must always be inspected for an lfm error element.
fn check_lfm_error(status: reqwest::StatusCode, body: &str) -> Result<()> {
    if !status.is_success() {
        if let Ok(error) = serde_json::from_str::<LastFmApiError>(body) {
            return Err(LastFmError::Api {
                code: error.error,
                message: error.message,
            });
        }
        return Err(LastFmError::Api {
            code: i32::from(status.as_u16()),
            message: body.to_string(),
        });
    }

    // Check for error in successful response
    if let Ok(error) = serde_json::from_str::<LastFmApiError>(body)
        && error.error != 0
    {
        return Err(LastFmError::Api {
            code: error.error,
            message: error.message,
        });
    }

    Ok(())
}

/// Inspect a track.scrobble response for filtered (ignored) scrobbles.
///
/// Last.fm answers ignored scrobbles with HTTP 200 and lfm status "ok",
/// so an accepted request is not necessarily a stored scrobble.
fn check_scrobble_accepted(body: &str) -> Result<()> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return Ok(());
    };

    let scrobbles = &value["scrobbles"];
    if scrobbles.is_null() {
        return Ok(());
    }

    if json_i64(&scrobbles["@attr"]["ignored"]).unwrap_or(0) == 0 {
        return Ok(());
    }

    let entry = match &scrobbles["scrobble"] {
        serde_json::Value::Array(list) => list.first(),
        serde_json::Value::Object(_) => Some(&scrobbles["scrobble"]),
        _ => None,
    };

    if let Some(message) = entry.map(|e| &e["ignoredMessage"])
        && !message.is_null()
    {
        return Err(LastFmError::Filtered {
            code: json_i64(&message["code"]).unwrap_or(0),
            message: message["#text"]
                .as_str()
                .unwrap_or("scrobble ignored")
                .to_string(),
        });
    }

    Err(LastFmError::Filtered {
        code: 0,
        message: "scrobble ignored without reason".to_string(),
    })
}

/// Last.fm JSON attributes are strings; accept both strings and numbers.
fn json_i64(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

/// Last.fm API error structure.
#[derive(Deserialize)]
struct LastFmApiError {
    error: i32,
    message: String,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{LastFmClient, LastFmClientInner, LastFmError, check_scrobble_accepted};

    #[test]
    fn client_clone_shares_same_inner_state() {
        let client = LastFmClient::new("test_key".into(), "test_secret".into()).unwrap();
        let cloned = client.clone();

        assert_eq!(client.api_key().unwrap(), cloned.api_key().unwrap());
        let LastFmClientInner::Live(client_inner) = &client.inner else {
            panic!("configured client must be live");
        };
        let LastFmClientInner::Live(cloned_inner) = &cloned.inner else {
            panic!("configured client clone must be live");
        };
        assert!(std::ptr::eq(client_inner.as_ref(), cloned_inner.as_ref()));
    }

    #[test]
    fn signature_is_alphabetical_concatenation_plus_secret() {
        // Vector from the Last.fm signature spec: params ordered by name,
        // concatenated without separators, secret appended, md5'd.
        // md5("api_keyfoomethodauth.getSessiontokenbazbar")
        let client = LastFmClient::new("foo".into(), "bar".into()).unwrap();
        let mut extra = BTreeMap::new();
        extra.insert("token".to_string(), "baz".to_string());

        let params = client.build_params("auth.getSession", None, extra).unwrap();

        assert_eq!(params["api_sig"], "b2f52fa6c54c2f0d0f8419e6cefb6edf");
        // format must be sent but never signed
        assert_eq!(params["format"], "json");
    }

    #[test]
    fn accepted_scrobble_is_ok() {
        let body = r##"{"scrobbles":{"@attr":{"accepted":"1","ignored":"0"},"scrobble":{"track":{"corrected":"0","#text":"Wanderlust"},"timestamp":"1288728745","ignoredMessage":{"code":"0","#text":""}}}}"##;
        assert!(check_scrobble_accepted(body).is_ok());
    }

    #[test]
    fn ignored_scrobble_is_filtered_error() {
        let body = r##"{"scrobbles":{"@attr":{"accepted":"0","ignored":"1"},"scrobble":{"artist":{"corrected":"0","#text":"Unknown Artist"},"timestamp":"1288728940","ignoredMessage":{"code":"1","#text":"Artist name failed filter: Unknown Artist"}}}}"##;

        let Err(LastFmError::Filtered { code, message }) = check_scrobble_accepted(body) else {
            panic!("ignored scrobble must be a filtered error");
        };
        assert_eq!(code, 1);
        assert!(message.contains("Unknown Artist"));
    }

    #[test]
    fn non_scrobble_body_is_ok() {
        assert!(check_scrobble_accepted("{}").is_ok());
        assert!(check_scrobble_accepted("not json").is_ok());
    }

    #[test]
    fn new_returns_disabled_when_credentials_are_empty() {
        assert!(matches!(
            LastFmClient::new(String::new(), "secret".into())
                .unwrap()
                .inner,
            LastFmClientInner::Disabled
        ));
        assert!(matches!(
            LastFmClient::new("key".into(), String::new())
                .unwrap()
                .inner,
            LastFmClientInner::Disabled
        ));
        assert!(matches!(
            LastFmClient::new(String::new(), String::new())
                .unwrap()
                .inner,
            LastFmClientInner::Disabled
        ));
    }
}
