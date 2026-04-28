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

        self.handle_response(response).await
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

        self.handle_response(response).await
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

    /// Handle API response and check for errors.
    async fn handle_response(&self, response: reqwest::Response) -> Result<()> {
        let status = response.status();
        let body: String = response.text().await?;

        if !status.is_success() {
            if let Ok(error) = serde_json::from_str::<LastFmApiError>(&body) {
                return Err(LastFmError::Api {
                    code: error.error,
                    message: error.message,
                });
            }
            return Err(LastFmError::Api {
                code: i32::from(status.as_u16()),
                message: body,
            });
        }

        // Check for error in successful response
        if let Ok(error) = serde_json::from_str::<LastFmApiError>(&body)
            && error.error != 0
        {
            return Err(LastFmError::Api {
                code: error.error,
                message: error.message,
            });
        }

        Ok(())
    }
}

/// Last.fm API error structure.
#[derive(Deserialize)]
struct LastFmApiError {
    error: i32,
    message: String,
}

#[cfg(test)]
mod tests {
    use super::{LastFmClient, LastFmClientInner};

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
        assert!(std::ptr::eq(&**client_inner, &**cloned_inner));
    }

    #[test]
    fn new_returns_disabled_when_credentials_are_empty() {
        assert!(matches!(
            LastFmClient::new("".into(), "secret".into()).unwrap().inner,
            LastFmClientInner::Disabled
        ));
        assert!(matches!(
            LastFmClient::new("key".into(), "".into()).unwrap().inner,
            LastFmClientInner::Disabled
        ));
        assert!(matches!(
            LastFmClient::new("".into(), "".into()).unwrap().inner,
            LastFmClientInner::Disabled
        ));
    }
}
