//! Authentication middleware and extractors for Subsonic API.
//!
//! Subsonic supports multiple authentication methods:
//! 1. Legacy: Plain password sent via `p` parameter (deprecated)
//! 2. Token: MD5(password + salt) sent via `t` and `s` parameters
//! 3. API Key (OpenSubsonic): API key sent via `apiKey` parameter
//!
//! For username/password auth, all API requests must include:
//! - `u`: Username
//! - `v`: Client API version
//! - `c`: Client name/identifier
//! - Either `p` (password) or `t` + `s` (token + salt)
//!
//! For API key auth:
//! - `apiKey`: The API key (must NOT include `u` parameter)
//! - `v`: Client API version
//! - `c`: Client name/identifier
//!
//! Parameters can be passed via:
//! - Query string (GET requests)
//! - Form body (POST requests with application/x-www-form-urlencoded)
//! - Or a combination of both (query params take precedence)

use std::sync::Arc;

use axum::{
    Form,
    body::Body,
    extract::{FromRef, FromRequest, Query, Request},
    http::Method,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use super::error::ApiError;
use super::response::{Format, error_response};
use crate::crypto::hash_password;
use crate::db::repo::ArtistInfoCacheRepository;
use crate::db::{
    AlbumRepository, ArtistRepository, DbPool, MusicFolderRepository, MusicRepoError,
    MusicRepoErrorKind, NewUser, NowPlayingEntry, NowPlayingRepository, PlayQueue,
    PlayQueueRepository, Playlist, PlaylistRepository, RatingRepository, RemoteCommand,
    RemoteControlRepository, RemoteSession, RemoteState, ScrobbleRepository, SongRepository,
    StarredRepository, UserRepoError, UserRepoErrorKind, UserRepository, UserUpdate,
};
use crate::lastfm::{LastFmClient, models::extract_biography, models::extract_image_urls};
use crate::models::User;
use crate::models::music::{Album, Artist, MusicFolder, Song};
use crate::models::user::UserRoles;
use crate::paths::resolve_cover_art_dir;
use crate::scanner::lyrics::ExtractedLyrics;
use crate::scanner::{ScanState, ScanStateHandle};
use chrono::NaiveDateTime;

/// User lookup required by authentication.
pub trait AuthState: Send + Sync + 'static {
    /// Find a user by username.
    fn find_user(&self, username: &str) -> Result<Option<User>, UserRepoError>;
    /// Find a user by API key.
    fn find_user_by_api_key(&self, api_key: &str) -> Result<Option<User>, UserRepoError>;
}

/// Shared authentication state handle.
#[derive(Clone)]
pub struct AuthStateHandle(Arc<DatabaseAuthState>);

impl AuthStateHandle {
    /// Create a shared state handle.
    #[must_use]
    pub const fn new(state: Arc<DatabaseAuthState>) -> Self {
        Self(state)
    }
}

impl std::fmt::Debug for AuthStateHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AuthStateHandle")
            .field(&"<dyn AuthState>")
            .finish()
    }
}

impl std::ops::Deref for AuthStateHandle {
    type Target = DatabaseAuthState;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

fn user_repo_error_to_music_repo_error(error: &UserRepoError) -> MusicRepoError {
    let kind = match error.kind() {
        UserRepoErrorKind::Database => MusicRepoErrorKind::Database,
        UserRepoErrorKind::Pool => MusicRepoErrorKind::Pool,
        UserRepoErrorKind::NotFound => MusicRepoErrorKind::NotFound,
        UserRepoErrorKind::UsernameExists => MusicRepoErrorKind::AlreadyExists,
    };
    MusicRepoError::new(kind, error.to_string())
}

#[expect(
    clippy::cast_possible_truncation,
    reason = "Subsonic album counts are bounded to signed 32-bit fields"
)]
pub(crate) fn saturating_i64_to_i32(value: i64) -> i32 {
    if value > i64::from(i32::MAX) {
        i32::MAX
    } else if value < i64::from(i32::MIN) {
        i32::MIN
    } else {
        value as i32
    }
}

/// Common query parameters for all Subsonic API requests.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AuthParams {
    /// Username
    #[serde(alias = "u")]
    pub u: String,
    /// Password (legacy, deprecated) - either hex-encoded with "enc:" prefix or plain
    #[serde(alias = "p")]
    pub p: Option<String>,
    /// Authentication token = MD5(password + salt)
    #[serde(alias = "t")]
    pub t: Option<String>,
    /// Salt used to generate the token
    #[serde(alias = "s")]
    pub s: Option<String>,
    /// API key (`OpenSubsonic` extension)
    #[serde(alias = "apiKey")]
    pub api_key: Option<String>,
    /// Client API version
    #[serde(alias = "v")]
    pub v: String,
    /// Client identifier
    #[serde(alias = "c")]
    pub c: String,
    /// Response format (xml, json, jsonp)
    #[serde(alias = "f")]
    pub f: Option<String>,
}

impl AuthParams {
    /// Get the response format.
    #[must_use]
    pub fn format(&self) -> Format {
        Format::from_param(self.f.as_deref())
    }

    /// Decode password if it's hex-encoded (prefixed with "enc:").
    #[must_use]
    pub fn decode_password(password: &str) -> Option<String> {
        password.strip_prefix("enc:").map_or_else(
            || Some(password.to_string()),
            |hex_encoded| {
                hex::decode(hex_encoded)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
            },
        )
    }

    /// Merge with another `AuthParams`, taking non-empty values from self.
    /// This is used to combine query params (higher priority) with form params.
    #[must_use]
    pub fn merge_with(mut self, other: Self) -> Self {
        if self.u.is_empty() {
            self.u = other.u;
        }
        if self.p.is_none() {
            self.p = other.p;
        }
        if self.t.is_none() {
            self.t = other.t;
        }
        if self.s.is_none() {
            self.s = other.s;
        }
        if self.api_key.is_none() {
            self.api_key = other.api_key;
        }
        if self.v.is_empty() {
            self.v = other.v;
        }
        if self.c.is_empty() {
            self.c = other.c;
        }
        if self.f.is_none() {
            self.f = other.f;
        }
        self
    }

    /// Check if API key auth is being used.
    #[must_use]
    pub const fn uses_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// Check if username/password auth is being used.
    #[must_use]
    pub const fn uses_user_auth(&self) -> bool {
        !self.u.is_empty() || self.p.is_some() || self.t.is_some()
    }
}

/// Authenticated user extractor that also includes the response format.
///
/// Supports GET and POST authentication parameters.
/// Endpoint parameters are still read from query strings.
///
/// Use this in your handlers to require authentication:
///
/// ```ignore
/// async fn handler(auth: SubsonicAuth) -> impl IntoResponse {
///     // auth.user is guaranteed to be authenticated
///     // auth.format contains the requested response format
///     ok_empty(auth.format)
/// }
/// ```
#[derive(Clone)]
pub struct SubsonicAuth {
    /// The authenticated user.
    pub user: User,
    /// The requested response format.
    pub format: Format,
    /// Common Subsonic authentication parameters.
    pub params: AuthParams,
    /// Reference to the auth state for accessing repositories.
    state: AuthStateHandle,
}

impl SubsonicAuth {
    /// Return shared database auth state.
    #[must_use]
    pub fn state(&self) -> &DatabaseAuthState {
        &self.state
    }
}

impl std::fmt::Debug for SubsonicAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubsonicAuth")
            .field("user", &self.user)
            .field("format", &self.format)
            .field("params", &self.params)
            .field("state", &"<dyn AuthState>")
            .finish()
    }
}

/// Error wrapper that includes format information for proper error responses.
#[derive(Debug)]
pub struct AuthError {
    pub error: ApiError,
    pub format: Format,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        error_response(self.format, &self.error).into_response()
    }
}

impl<S> FromRequest<S> for SubsonicAuth
where
    S: Send + Sync,
    AuthStateHandle: FromRef<S>,
{
    type Rejection = AuthError;

    #[expect(
        clippy::too_many_lines,
        reason = "Extractor validates multiple auth flows and transports in one place"
    )]
    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let is_post = req.method() == Method::POST;

        // Extract query parameters first (they exist in both GET and POST)
        let (parts, body) = req.into_parts();
        let query_params = Query::<AuthParams>::try_from_uri(&parts.uri)
            .map(|q| q.0)
            .unwrap_or_default();

        // For POST requests, also extract form body parameters
        let mut params = if is_post {
            // Reconstruct the request to extract form data
            let req = Request::from_parts(parts.clone(), body);
            match Form::<AuthParams>::from_request(req, state).await {
                Ok(Form(form_params)) => query_params.merge_with(form_params),
                Err(e) => {
                    tracing::warn!(error = %e, "form auth parameter parsing failed");
                    return Err(AuthError {
                        error: ApiError::MissingParameter("valid form body".into()),
                        format: query_params.format(),
                    });
                }
            }
        } else {
            query_params
        };

        // Support for clients passing credentials in HTTP headers (e.g. SolidSonic)
        // Checks for X-Subsonic-Username, X-Subsonic-Token, and X-Subsonic-Salt
        #[expect(
            clippy::collapsible_if,
            reason = "Nested checks keep header parsing flow explicit"
        )]
        if params.u.is_empty() {
            if let Some(Ok(u)) = parts.headers.get("X-Subsonic-Username").map(|h| h.to_str()) {
                params.u = u.to_string();

                if let Some(Ok(t)) = parts.headers.get("X-Subsonic-Token").map(|h| h.to_str()) {
                    params.t = Some(t.to_string());
                }

                if let Some(Ok(s)) = parts.headers.get("X-Subsonic-Salt").map(|h| h.to_str()) {
                    params.s = Some(s.to_string());
                }
            }
        }

        let format = params.format();

        // Validate common required parameters (for all auth methods)
        if params.v.is_empty() {
            return Err(AuthError {
                error: ApiError::MissingParameter("v (version)".into()),
                format,
            });
        }
        if params.c.is_empty() {
            return Err(AuthError {
                error: ApiError::MissingParameter("c (client)".into()),
                format,
            });
        }

        // Get auth state
        let auth_state = AuthStateHandle::from_ref(state);

        // Check for conflicting auth mechanisms
        if params.uses_api_key() && params.uses_user_auth() {
            return Err(AuthError {
                error: ApiError::ConflictingAuthMechanisms,
                format,
            });
        }

        // Authenticate based on the method used
        if let Some(api_key) = &params.api_key {
            // API Key authentication (OpenSubsonic extension)
            // When using API key, username must NOT be provided
            if !params.u.is_empty() {
                return Err(AuthError {
                    error: ApiError::ConflictingAuthMechanisms,
                    format,
                });
            }

            let user = auth_state
                .find_user_by_api_key(api_key)
                .map_err(|error| AuthError {
                    error: ApiError::Generic(error.to_string()),
                    format,
                })?
                .ok_or(AuthError {
                    error: ApiError::InvalidApiKey,
                    format,
                })?;

            Ok(Self {
                user,
                format,
                params,
                state: auth_state,
            })
        } else {
            // Username/password or token authentication
            if params.u.is_empty() {
                return Err(AuthError {
                    error: ApiError::MissingParameter("u (username) or apiKey".into()),
                    format,
                });
            }

            // Find user by username
            let user = auth_state
                .find_user(&params.u)
                .map_err(|error| AuthError {
                    error: ApiError::Generic(error.to_string()),
                    format,
                })?
                .ok_or(AuthError {
                    error: ApiError::WrongCredentials,
                    format,
                })?;

            // Authenticate using token or password
            let authenticated = if let (Some(token), Some(salt)) = (&params.t, &params.s) {
                // Token authentication (preferred by many clients)
                user.verify_token(token, salt)
            } else if let Some(password) = &params.p {
                // Legacy password authentication - use Argon2
                AuthParams::decode_password(password)
                    .is_some_and(|decoded| user.verify_password(&decoded))
            } else {
                return Err(AuthError {
                    error: ApiError::MissingParameter(
                        "authentication: 'apiKey', 'p' (password), or 't' and 's' (token and salt)"
                            .into(),
                    ),
                    format,
                });
            };

            if authenticated {
                Ok(Self {
                    user,
                    format,
                    params,
                    state: auth_state,
                })
            } else {
                Err(AuthError {
                    error: ApiError::WrongCredentials,
                    format,
                })
            }
        }
    }
}

/// Database-backed authentication state.
///
/// Uses the user repository to look up users from `SQLite`.
#[derive(Clone, Debug)]
pub struct DatabaseAuthState {
    pool: DbPool,
    user_repo: UserRepository,
    music_folder_repo: MusicFolderRepository,
    artist_repo: ArtistRepository,
    album_repo: AlbumRepository,
    song_repo: SongRepository,
    starred_repo: StarredRepository,
    now_playing_repo: NowPlayingRepository,
    scrobble_repo: ScrobbleRepository,
    rating_repo: RatingRepository,
    playlist_repo: PlaylistRepository,
    play_queue_repo: PlayQueueRepository,
    remote_control_repo: RemoteControlRepository,
    artist_cache_repo: ArtistInfoCacheRepository,
    scan_state: ScanStateHandle,
    lastfm_client: Option<LastFmClient>,
}

impl DatabaseAuthState {
    /// Create a new database auth state with its own scan state.
    #[must_use]
    pub fn new(pool: DbPool) -> Self {
        Self::with_scan_state(pool, ScanStateHandle::new(ScanState::new()), None)
    }

    /// Create a new database auth state with a shared scan state and optional Last.fm client.
    #[must_use]
    pub fn with_scan_state(
        pool: DbPool,
        scan_state: ScanStateHandle,
        lastfm_client: Option<LastFmClient>,
    ) -> Self {
        Self {
            pool: pool.clone(),
            user_repo: UserRepository::new(pool.clone()),
            music_folder_repo: MusicFolderRepository::new(pool.clone()),
            artist_repo: ArtistRepository::new(pool.clone()),
            album_repo: AlbumRepository::new(pool.clone()),
            song_repo: SongRepository::new(pool.clone()),
            starred_repo: StarredRepository::new(pool.clone()),
            now_playing_repo: NowPlayingRepository::new(pool.clone()),
            scrobble_repo: ScrobbleRepository::new(pool.clone()),
            rating_repo: RatingRepository::new(pool.clone()),
            playlist_repo: PlaylistRepository::new(pool.clone()),
            play_queue_repo: PlayQueueRepository::new(pool.clone()),
            remote_control_repo: RemoteControlRepository::new(pool.clone()),
            artist_cache_repo: ArtistInfoCacheRepository::new(pool),
            scan_state,
            lastfm_client,
        }
    }

    /// Get a reference to the user repository.
    #[must_use]
    pub const fn user_repo(&self) -> &UserRepository {
        &self.user_repo
    }

    /// Get a reference to the music folder repository.
    #[must_use]
    pub const fn music_folder_repo(&self) -> &MusicFolderRepository {
        &self.music_folder_repo
    }

    /// Submit a scrobble to Last.fm in the background.
    pub(crate) fn submit_lastfm_scrobble(
        &self,
        user_id: i32,
        song: &crate::models::music::Song,
        timestamp: i64,
    ) {
        if let Some(client) = &self.lastfm_client
            && let Ok(Some(session_key)) = self.user_repo.get_lastfm_session_key(user_id)
        {
            let client = client.clone();
            let artist = song.artist_name.clone().unwrap_or_default();
            let track = song.title.clone();
            let album = song.album_name.clone();

            tokio::spawn(async move {
                if let Err(e) = client
                    .scrobble(&session_key, &artist, &track, album.as_deref(), timestamp)
                    .await
                {
                    tracing::warn!(error = %e, "Failed to submit scrobble to Last.fm");
                } else {
                    tracing::debug!(artist = %artist, track = %track, "Submitted scrobble to Last.fm");
                }
            });
        }
    }

    /// Update Last.fm now playing in the background.
    pub(crate) fn update_lastfm_now_playing(
        &self,
        user_id: i32,
        song: &crate::models::music::Song,
    ) {
        if let Some(client) = &self.lastfm_client
            && let Ok(Some(session_key)) = self.user_repo.get_lastfm_session_key(user_id)
        {
            let client = client.clone();
            let artist = song.artist_name.clone().unwrap_or_default();
            let track = song.title.clone();
            let album = song.album_name.clone();
            let duration = Some(song.duration);

            tokio::spawn(async move {
                if let Err(e) = client
                    .update_now_playing(&session_key, &artist, &track, album.as_deref(), duration)
                    .await
                {
                    tracing::debug!(error = %e, "Failed to update Last.fm now playing");
                } else {
                    tracing::debug!(artist = %artist, track = %track, "Updated Last.fm now playing");
                }
            });
        }
    }
}

impl DatabaseAuthState {
    pub(crate) fn find_user(&self, username: &str) -> Result<Option<User>, UserRepoError> {
        self.user_repo.find_by_username(username)
    }

    pub(crate) fn find_user_by_api_key(
        &self,
        api_key: &str,
    ) -> Result<Option<User>, UserRepoError> {
        self.user_repo.find_by_api_key(api_key)
    }

    pub(crate) fn get_music_folders(&self) -> Result<Vec<MusicFolder>, MusicRepoError> {
        self.music_folder_repo.find_enabled()
    }

    pub(crate) fn get_artists(&self) -> Result<Vec<Artist>, MusicRepoError> {
        self.artist_repo.find_all()
    }

    pub(crate) fn get_artists_last_modified(
        &self,
    ) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        self.artist_repo.get_last_modified()
    }

    pub(crate) fn get_artist_album_count(&self, artist_id: i32) -> Result<i64, MusicRepoError> {
        self.artist_repo.count_albums(artist_id)
    }

    pub(crate) fn get_song(&self, song_id: i32) -> Result<Option<Song>, MusicRepoError> {
        self.song_repo.find_by_id(song_id)
    }

    pub(crate) fn find_song_by_artist_and_title(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<Option<Song>, MusicRepoError> {
        self.song_repo.find_by_artist_and_title(artist, title)
    }

    pub(crate) fn get_album(&self, album_id: i32) -> Result<Option<Album>, MusicRepoError> {
        self.album_repo.find_by_id(album_id)
    }

    pub(crate) fn get_artist(&self, artist_id: i32) -> Result<Option<Artist>, MusicRepoError> {
        self.artist_repo.find_by_id(artist_id)
    }

    pub(crate) fn get_songs_by_album(&self, album_id: i32) -> Result<Vec<Song>, MusicRepoError> {
        self.song_repo.find_by_album(album_id)
    }

    pub(crate) fn get_albums_by_artist(
        &self,
        artist_id: i32,
    ) -> Result<Vec<Album>, MusicRepoError> {
        self.album_repo.find_by_artist(artist_id)
    }

    pub(crate) fn get_albums_alphabetical_by_name(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        self.album_repo.find_alphabetical_by_name(offset, limit)
    }

    pub(crate) fn get_albums_alphabetical_by_artist(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        self.album_repo.find_alphabetical_by_artist(offset, limit)
    }

    pub(crate) fn get_albums_newest(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        self.album_repo.find_newest(offset, limit)
    }

    pub(crate) fn get_albums_frequent(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        self.album_repo.find_frequent(offset, limit)
    }

    pub(crate) fn get_albums_recent(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        self.album_repo.find_recent(offset, limit)
    }

    pub(crate) fn get_albums_random(&self, limit: i64) -> Result<Vec<Album>, MusicRepoError> {
        self.album_repo.find_random(limit)
    }

    pub(crate) fn get_albums_by_year(
        &self,
        from_year: i32,
        to_year: i32,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        self.album_repo
            .find_by_year_range(from_year, to_year, offset, limit)
    }

    pub(crate) fn get_albums_by_genre(
        &self,
        genre: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        self.album_repo.find_by_genre(genre, offset, limit)
    }

    pub(crate) fn get_albums_starred(
        &self,
        user_id: i32,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        let starred = self
            .starred_repo
            .get_starred_albums_paginated(user_id, offset, limit)?;
        Ok(starred.into_iter().map(|(album, _)| album).collect())
    }

    pub(crate) fn get_albums_highest(
        &self,
        user_id: i32,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        // Get highest rated album IDs
        let album_ids = self
            .rating_repo
            .get_highest_rated_album_ids(user_id, offset, limit)?;

        if album_ids.is_empty() {
            return Ok(vec![]);
        }

        // Fetch albums and maintain order
        let albums = self.album_repo.find_by_ids(&album_ids)?;

        // Re-order albums to match the rating order
        let mut album_map: std::collections::HashMap<i32, Album> =
            albums.into_iter().map(|a| (a.id, a)).collect();

        Ok(album_ids
            .into_iter()
            .filter_map(|id| album_map.remove(&id))
            .collect())
    }

    pub(crate) fn get_genres(&self) -> Result<Vec<(String, i64, i64)>, MusicRepoError> {
        self.song_repo.get_genres()
    }

    pub(crate) fn search_artists(
        &self,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Artist>, MusicRepoError> {
        self.artist_repo.search(query, offset, limit)
    }

    pub(crate) fn search_albums(
        &self,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        self.album_repo.search(query, offset, limit)
    }

    pub(crate) fn search_songs(
        &self,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Song>, MusicRepoError> {
        self.song_repo.search(query, offset, limit)
    }

    pub(crate) fn star_artist(&self, user_id: i32, artist_id: i32) -> Result<(), MusicRepoError> {
        self.starred_repo.star_artist(user_id, artist_id)
    }

    pub(crate) fn star_album(&self, user_id: i32, album_id: i32) -> Result<(), MusicRepoError> {
        self.starred_repo.star_album(user_id, album_id)
    }

    pub(crate) fn star_song(&self, user_id: i32, song_id: i32) -> Result<(), MusicRepoError> {
        self.starred_repo.star_song(user_id, song_id)
    }

    pub(crate) fn unstar_artist(&self, user_id: i32, artist_id: i32) -> Result<(), MusicRepoError> {
        self.starred_repo
            .unstar_artist(user_id, artist_id)
            .map(|_| ())
    }

    pub(crate) fn unstar_album(&self, user_id: i32, album_id: i32) -> Result<(), MusicRepoError> {
        self.starred_repo
            .unstar_album(user_id, album_id)
            .map(|_| ())
    }

    pub(crate) fn unstar_song(&self, user_id: i32, song_id: i32) -> Result<(), MusicRepoError> {
        self.starred_repo.unstar_song(user_id, song_id).map(|_| ())
    }

    pub(crate) fn get_starred_artists(
        &self,
        user_id: i32,
    ) -> Result<Vec<(Artist, NaiveDateTime)>, MusicRepoError> {
        self.starred_repo.get_starred_artists(user_id)
    }

    pub(crate) fn get_starred_albums(
        &self,
        user_id: i32,
    ) -> Result<Vec<(Album, NaiveDateTime)>, MusicRepoError> {
        self.starred_repo.get_starred_albums(user_id)
    }

    pub(crate) fn get_starred_songs(
        &self,
        user_id: i32,
    ) -> Result<Vec<(Song, NaiveDateTime)>, MusicRepoError> {
        self.starred_repo.get_starred_songs(user_id)
    }

    pub(crate) fn get_starred_at_for_artist(
        &self,
        user_id: i32,
        artist_id: i32,
    ) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        self.starred_repo
            .get_starred_at_for_artist(user_id, artist_id)
    }

    pub(crate) fn get_starred_at_for_album(
        &self,
        user_id: i32,
        album_id: i32,
    ) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        self.starred_repo
            .get_starred_at_for_album(user_id, album_id)
    }

    pub(crate) fn get_starred_at_for_song(
        &self,
        user_id: i32,
        song_id: i32,
    ) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        self.starred_repo.get_starred_at_for_song(user_id, song_id)
    }

    pub(crate) fn get_artist_album_counts_batch(
        &self,
        artist_ids: &[i32],
    ) -> Result<std::collections::HashMap<i32, i64>, MusicRepoError> {
        self.artist_repo.count_albums_batch(artist_ids)
    }

    pub(crate) fn get_starred_at_for_songs_batch(
        &self,
        user_id: i32,
        song_ids: &[i32],
    ) -> Result<std::collections::HashMap<i32, NaiveDateTime>, MusicRepoError> {
        self.starred_repo
            .get_starred_at_for_songs_batch(user_id, song_ids)
    }

    pub(crate) fn get_starred_at_for_albums_batch(
        &self,
        user_id: i32,
        album_ids: &[i32],
    ) -> Result<std::collections::HashMap<i32, NaiveDateTime>, MusicRepoError> {
        self.starred_repo
            .get_starred_at_for_albums_batch(user_id, album_ids)
    }

    pub(crate) fn get_starred_at_for_artists_batch(
        &self,
        user_id: i32,
        artist_ids: &[i32],
    ) -> Result<std::collections::HashMap<i32, NaiveDateTime>, MusicRepoError> {
        self.starred_repo
            .get_starred_at_for_artists_batch(user_id, artist_ids)
    }

    pub(crate) fn scrobble(
        &self,
        user_id: i32,
        song_id: i32,
        time: Option<i64>,
        submission: bool,
    ) -> Result<(), MusicRepoError> {
        // Record scrobble locally
        self.scrobble_repo
            .scrobble(user_id, song_id, time, submission)?;

        // Submit to Last.fm if configured and this is a real submission (not "now playing")
        if submission && let Some(song) = self.get_song(song_id)? {
            let timestamp = time.unwrap_or_else(|| chrono::Utc::now().timestamp());
            self.submit_lastfm_scrobble(user_id, &song, timestamp);
        }

        Ok(())
    }

    pub(crate) fn set_now_playing(
        &self,
        user_id: i32,
        song_id: i32,
        player_id: Option<&str>,
    ) -> Result<(), MusicRepoError> {
        // Record locally
        self.now_playing_repo
            .set_now_playing(user_id, song_id, player_id)?;

        // Update Last.fm now playing if configured
        if let Some(song) = self.get_song(song_id)? {
            self.update_lastfm_now_playing(user_id, &song);
        }

        Ok(())
    }

    pub(crate) fn get_now_playing(&self) -> Result<Vec<NowPlayingEntry>, MusicRepoError> {
        self.now_playing_repo.get_all_now_playing()
    }

    pub(crate) fn get_random_songs(
        &self,
        size: i64,
        genre: Option<&str>,
        from_year: Option<i32>,
        to_year: Option<i32>,
        music_folder_id: Option<i32>,
    ) -> Result<Vec<Song>, MusicRepoError> {
        self.song_repo
            .find_random(size, genre, from_year, to_year, music_folder_id)
    }

    pub(crate) fn get_songs_by_genre(
        &self,
        genre: &str,
        count: i64,
        offset: i64,
        music_folder_id: Option<i32>,
    ) -> Result<Vec<Song>, MusicRepoError> {
        self.song_repo
            .find_by_genre(genre, count, offset, music_folder_id)
    }

    pub(crate) fn set_song_rating(
        &self,
        user_id: i32,
        song_id: i32,
        rating: i32,
    ) -> Result<(), MusicRepoError> {
        self.rating_repo.set_song_rating(user_id, song_id, rating)
    }

    pub(crate) fn get_playlists(
        &self,
        user_id: i32,
        username: &str,
    ) -> Result<Vec<Playlist>, MusicRepoError> {
        self.playlist_repo.get_playlists(user_id, username)
    }

    pub(crate) fn get_playlist(
        &self,
        playlist_id: i32,
    ) -> Result<Option<Playlist>, MusicRepoError> {
        self.playlist_repo.get_playlist(playlist_id)
    }

    pub(crate) fn get_playlist_songs(&self, playlist_id: i32) -> Result<Vec<Song>, MusicRepoError> {
        self.playlist_repo.get_playlist_songs(playlist_id)
    }

    pub(crate) fn get_playlist_cover_arts_batch(
        &self,
        playlist_ids: &[i32],
    ) -> Result<std::collections::HashMap<i32, String>, MusicRepoError> {
        self.playlist_repo
            .get_playlist_cover_arts_batch(playlist_ids)
    }

    pub(crate) fn create_playlist(
        &self,
        user_id: i32,
        name: &str,
        comment: Option<&str>,
        song_ids: &[i32],
    ) -> Result<Playlist, MusicRepoError> {
        self.playlist_repo
            .create_playlist(user_id, name, comment, song_ids)
    }

    pub(crate) fn update_playlist(
        &self,
        playlist_id: i32,
        name: Option<&str>,
        comment: Option<&str>,
        public: Option<bool>,
        song_ids_to_add: &[i32],
        song_indices_to_remove: &[i32],
    ) -> Result<(), MusicRepoError> {
        self.playlist_repo.update_playlist(
            playlist_id,
            name,
            comment,
            public,
            song_ids_to_add,
            song_indices_to_remove,
        )
    }

    pub(crate) fn delete_playlist(&self, playlist_id: i32) -> Result<bool, MusicRepoError> {
        self.playlist_repo.delete_playlist(playlist_id)
    }

    pub(crate) fn is_playlist_owner(
        &self,
        user_id: i32,
        playlist_id: i32,
    ) -> Result<bool, MusicRepoError> {
        self.playlist_repo.is_owner(user_id, playlist_id)
    }

    pub(crate) fn get_play_queue(
        &self,
        user_id: i32,
        username: &str,
    ) -> Result<Option<PlayQueue>, MusicRepoError> {
        self.play_queue_repo.get_play_queue(user_id, username)
    }

    pub(crate) fn save_play_queue(
        &self,
        user_id: i32,
        song_ids: &[i32],
        current_song_id: Option<i32>,
        position: Option<i64>,
        changed_by: Option<&str>,
    ) -> Result<(), MusicRepoError> {
        self.play_queue_repo.save_play_queue(
            user_id,
            song_ids,
            current_song_id,
            position,
            changed_by,
        )
    }

    pub(crate) fn create_remote_session(
        &self,
        user_id: i32,
        host_device_id: &str,
        host_device_name: Option<&str>,
        ttl_seconds: i64,
    ) -> Result<RemoteSession, MusicRepoError> {
        self.remote_control_repo.create_session(
            user_id,
            host_device_id,
            host_device_name,
            ttl_seconds,
        )
    }

    pub(crate) fn join_remote_session(
        &self,
        user_id: i32,
        pairing_code: &str,
        controller_device_id: &str,
        controller_device_name: Option<&str>,
    ) -> Result<Option<RemoteSession>, MusicRepoError> {
        self.remote_control_repo.join_session(
            pairing_code,
            user_id,
            controller_device_id,
            controller_device_name,
        )
    }

    pub(crate) fn close_remote_session(
        &self,
        user_id: i32,
        session_id: &str,
    ) -> Result<bool, MusicRepoError> {
        self.remote_control_repo.close_session(session_id, user_id)
    }

    pub(crate) fn get_remote_session(
        &self,
        user_id: i32,
        session_id: &str,
    ) -> Result<Option<RemoteSession>, MusicRepoError> {
        self.remote_control_repo
            .get_session_for_user(session_id, user_id)
    }

    pub(crate) fn send_remote_command(
        &self,
        user_id: i32,
        session_id: &str,
        source_device_id: &str,
        command: &str,
        payload: Option<&str>,
    ) -> Result<i64, MusicRepoError> {
        self.remote_control_repo
            .get_session_for_user(session_id, user_id)?
            .ok_or_else(|| {
                MusicRepoError::new(MusicRepoErrorKind::NotFound, "remote session not found")
            })?;

        self.remote_control_repo
            .enqueue_command(session_id, source_device_id, command, payload)
    }

    pub(crate) fn get_remote_commands(
        &self,
        user_id: i32,
        session_id: &str,
        since_id: i64,
        limit: i64,
        exclude_device_id: &str,
    ) -> Result<Vec<RemoteCommand>, MusicRepoError> {
        self.remote_control_repo
            .get_session_for_user(session_id, user_id)?
            .ok_or_else(|| {
                MusicRepoError::new(MusicRepoErrorKind::NotFound, "remote session not found")
            })?;

        self.remote_control_repo
            .get_commands(session_id, since_id, limit, exclude_device_id)
    }

    pub(crate) fn update_remote_state(
        &self,
        user_id: i32,
        session_id: &str,
        updated_by_device_id: &str,
        state_json: &str,
    ) -> Result<(), MusicRepoError> {
        self.remote_control_repo
            .get_session_for_user(session_id, user_id)?
            .ok_or_else(|| {
                MusicRepoError::new(MusicRepoErrorKind::NotFound, "remote session not found")
            })?;

        self.remote_control_repo
            .update_state(session_id, updated_by_device_id, state_json)
    }

    pub(crate) fn get_remote_state(
        &self,
        user_id: i32,
        session_id: &str,
    ) -> Result<Option<RemoteState>, MusicRepoError> {
        self.remote_control_repo
            .get_session_for_user(session_id, user_id)?
            .ok_or_else(|| {
                MusicRepoError::new(MusicRepoErrorKind::NotFound, "remote session not found")
            })?;

        self.remote_control_repo.get_state(session_id)
    }

    pub(crate) fn get_user(&self, username: &str) -> Result<Option<User>, UserRepoError> {
        self.user_repo.find_by_username(username)
    }

    pub(crate) fn get_all_users(&self) -> Result<Vec<User>, UserRepoError> {
        self.user_repo.find_all()
    }

    pub(crate) fn delete_user(&self, username: &str) -> Result<bool, UserRepoError> {
        let user = self.user_repo.find_by_username(username)?.ok_or_else(|| {
            UserRepoError::new(
                UserRepoErrorKind::NotFound,
                format!("user '{username}' not found"),
            )
        })?;
        self.user_repo.delete(user.id)
    }

    pub(crate) fn change_password(
        &self,
        username: &str,
        new_password: &str,
    ) -> Result<(), UserRepoError> {
        let user = self.user_repo.find_by_username(username)?.ok_or_else(|| {
            UserRepoError::new(
                UserRepoErrorKind::NotFound,
                format!("user '{username}' not found"),
            )
        })?;

        let password_hash = hash_password(new_password)
            .map_err(|error| UserRepoError::new(UserRepoErrorKind::Database, error.to_string()))?;

        self.user_repo.update_password(user.id, &password_hash)?;

        // Also update the subsonic_password for token auth compatibility
        self.user_repo
            .update_subsonic_password(user.id, new_password)?;

        Ok(())
    }

    pub(crate) fn create_user(
        &self,
        username: &str,
        password: &str,
        email: &str,
        roles: &UserRoles,
    ) -> Result<User, UserRepoError> {
        let password_hash = hash_password(password)
            .map_err(|error| UserRepoError::new(UserRepoErrorKind::Database, error.to_string()))?;

        let new_user = NewUser::builder(username, &password_hash)
            .subsonic_password(password)
            .email(email)
            .admin_role(roles.admin_role)
            .settings_role(roles.settings_role)
            .stream_role(roles.stream_role)
            .jukebox_role(roles.jukebox_role)
            .download_role(roles.download_role)
            .upload_role(roles.upload_role)
            .playlist_role(roles.playlist_role)
            .cover_art_role(roles.cover_art_role)
            .comment_role(roles.comment_role)
            .podcast_role(roles.podcast_role)
            .share_role(roles.share_role)
            .video_conversion_role(roles.video_conversion_role)
            .max_bit_rate(0)
            .build();

        self.user_repo.create(&new_user)
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "Subsonic user updates expose many independently optional role fields"
    )]
    pub(crate) fn update_user(
        &self,
        username: &str,
        password: Option<&str>,
        email: Option<&str>,
        admin_role: Option<bool>,
        settings_role: Option<bool>,
        stream_role: Option<bool>,
        jukebox_role: Option<bool>,
        download_role: Option<bool>,
        upload_role: Option<bool>,
        playlist_role: Option<bool>,
        cover_art_role: Option<bool>,
        comment_role: Option<bool>,
        podcast_role: Option<bool>,
        share_role: Option<bool>,
        video_conversion_role: Option<bool>,
        max_bit_rate: Option<i32>,
    ) -> Result<(), UserRepoError> {
        // If password is being updated, update that first
        if let Some(pwd) = password {
            self.change_password(username, pwd)?;
        }

        // Build update struct
        let update = UserUpdate {
            username: username.to_string(),
            email: email.map(std::string::ToString::to_string),
            admin_role,
            settings_role,
            stream_role,
            jukebox_role,
            download_role,
            upload_role,
            playlist_role,
            cover_art_role,
            comment_role,
            podcast_role,
            share_role,
            video_conversion_role,
            max_bit_rate,
            lastfm_session_key: None, // Not updated through this method
        };

        self.user_repo.update_user(&update)?;
        Ok(())
    }

    pub(crate) fn get_db_pool(&self) -> DbPool {
        self.pool.clone()
    }

    pub(crate) fn get_scan_state(&self) -> ScanStateHandle {
        self.scan_state.clone()
    }

    pub(crate) fn get_similar_songs_by_artist(
        &self,
        artist_id: i32,
        exclude_song_id: i32,
        limit: i64,
    ) -> Result<Vec<Song>, MusicRepoError> {
        self.song_repo
            .find_random_by_artist(artist_id, exclude_song_id, limit)
    }

    pub(crate) fn get_top_songs_by_artist_name(
        &self,
        artist_name: &str,
        limit: i64,
    ) -> Result<Vec<Song>, MusicRepoError> {
        self.song_repo.find_top_by_artist_name(artist_name, limit)
    }

    pub(crate) fn get_song_lyrics(
        &self,
        song_id: i32,
    ) -> Result<Vec<ExtractedLyrics>, MusicRepoError> {
        use crate::scanner::lyrics::extract_lyrics;
        use std::path::Path;

        // Get the song to find its file path
        let Some(song) = self.get_song(song_id)? else {
            return Ok(Vec::new());
        };

        // Extract lyrics from the audio file
        Ok(extract_lyrics(Path::new(&song.path)))
    }

    #[expect(
        clippy::too_many_lines,
        reason = "This method coordinates cache read/write and async Last.fm enrichment"
    )]
    pub(crate) fn get_artist_info_with_cache(
        &self,
        artist_id: i32,
    ) -> Result<crate::models::music::ArtistInfo2Response, MusicRepoError> {
        use crate::lastfm::models::LastFmArtistCache;
        use crate::models::music::{ArtistID3Response, ArtistInfo2Response};
        use tokio::io::AsyncWriteExt;

        // Get the artist from the database
        let Some(artist) = self.get_artist(artist_id)? else {
            return Ok(ArtistInfo2Response::empty());
        };

        // Start with basic info from the artist record
        let mut response = ArtistInfo2Response::from_artist(&artist);

        tracing::debug!(artist_id = artist_id, artist = %artist.name, "Fetching artist info");

        // Try to get cached Last.fm data
        match self
            .artist_cache_repo
            .get_valid_cache(artist_id)
            .map_err(|ref e| user_repo_error_to_music_repo_error(e))?
        {
            Some(cache) => {
                tracing::debug!(artist_id = artist_id, "Using cached Last.fm info");
                // Use cached data
                response.biography = cache.biography;
                response.last_fm_url = cache.last_fm_url;
                response.small_image_url = cache.small_image_url;
                response.medium_image_url = cache.medium_image_url;
                response.large_image_url = cache.large_image_url;

                // Try to find similar artists by name
                for similar_name in &cache.similar_artists {
                    if let Some(similar_artist) = self.artist_repo.find_by_name(similar_name)? {
                        let album_count = self.get_artist_album_count(similar_artist.id)?;
                        response
                            .similar_artists
                            .push(ArtistID3Response::from_artist(
                                &similar_artist,
                                Some(saturating_i64_to_i32(album_count)),
                            ));
                    }
                }
            }
            None => {
                // No valid cache, try to fetch from Last.fm if configured
                if let Some(client) = &self.lastfm_client {
                    let client = client.clone();
                    let artist_name = artist.name;
                    let artist_id_copy = artist_id;
                    let cache_repo = self.artist_cache_repo.clone();
                    let artist_repo = self.artist_repo.clone();

                    // Spawn async task to fetch, cache, and download image
                    tokio::spawn(async move {
                        match client.get_artist_info(&artist_name).await {
                            Ok(Some(lastfm_artist)) => {
                                let (mut small, mut medium, mut large) =
                                    extract_image_urls(&lastfm_artist.image);

                                // Always try to scrape the artist page for the best image (og:image)
                                // This aligns with other Subsonic servers (Gonic, Navidrome) as Last.fm API
                                // often returns placeholders or lower quality images compared to the web page.
                                if let Some(ref page_url) = lastfm_artist.url {
                                    tracing::debug!(
                                        artist = %artist_name,
                                        url = %page_url,
                                        "Attempting to scrape artist image from page"
                                    );
                                    match client.fetch_artist_image_from_page(page_url).await {
                                        Ok(Some(scraped_url)) => {
                                            tracing::debug!(
                                                artist = %artist_name,
                                                url = %scraped_url,
                                                "Successfully scraped artist image"
                                            );
                                            large = Some(scraped_url);
                                            // We don't have small/medium for scraped image usually, so just use large
                                            if small.is_none() {
                                                small = large.clone();
                                            }
                                            if medium.is_none() {
                                                medium = large.clone();
                                            }
                                        }
                                        Ok(None) => {
                                            tracing::debug!(
                                                artist = %artist_name,
                                                "No image found on scraped page"
                                            );
                                        }
                                        Err(e) => {
                                            tracing::warn!(
                                                error = %e,
                                                artist = %artist_name,
                                                "Failed to scrape artist page"
                                            );
                                        }
                                    }
                                }

                                tracing::debug!(
                                    artist = %artist_name,
                                    small = ?small,
                                    medium = ?medium,
                                    large = ?large,
                                    "Final Last.fm image URLs"
                                );
                                let bio = extract_biography(&lastfm_artist.bio);

                                // Get similar artist names
                                let similar_names: Vec<String> = lastfm_artist
                                    .similar
                                    .artist
                                    .iter()
                                    .map(|a| a.name.clone())
                                    .collect();

                                let cache = LastFmArtistCache {
                                    artist_id: artist_id_copy,
                                    biography: bio,
                                    last_fm_url: lastfm_artist.url,
                                    small_image_url: small,
                                    medium_image_url: medium,
                                    large_image_url: large.clone(),
                                    similar_artists: similar_names,
                                    updated_at: chrono::Local::now().naive_local(),
                                };

                                if let Err(e) = cache_repo.save_cache(&cache) {
                                    tracing::warn!(error = %e, "Failed to save artist cache");
                                } else {
                                    tracing::debug!(artist = %artist_name, "Cached Last.fm artist info");
                                }

                                // Download and persist artist image if available
                                if let Some(image_url) = large {
                                    // Check for known Last.fm placeholder (star image)
                                    // The known generic "star" image from Last.fm often has this hash in the URL
                                    // Example: https://lastfm.freetls.fastly.net/i/u/300x300/2a96cbd8b46e442fc41c2b86b821562f.png
                                    if image_url.contains("2a96cbd8b46e442fc41c2b86b821562f") {
                                        tracing::warn!(
                                            artist = %artist_name,
                                            url = %image_url,
                                            "Skipping Last.fm placeholder image"
                                        );
                                        return;
                                    }

                                    let cover_art_dir = resolve_cover_art_dir();

                                    // Ensure directory exists
                                    if !cover_art_dir.exists() {
                                        let _ = tokio::fs::create_dir_all(&cover_art_dir).await;
                                    }

                                    // Determine extension
                                    let ext = if image_url.to_lowercase().ends_with(".png") {
                                        "png"
                                    } else if image_url.to_lowercase().ends_with(".gif") {
                                        "gif"
                                    } else {
                                        "jpg"
                                    };

                                    let cover_art_id = format!("artist-{artist_id_copy}");
                                    let filename = format!("{cover_art_id}.{ext}");
                                    let filepath = cover_art_dir.join(&filename);

                                    // Check if we need to download (skip if exists)
                                    if !filepath.exists() {
                                        match reqwest::get(&image_url).await {
                                            Ok(resp) => {
                                                if resp.status().is_success() {
                                                    match resp.bytes().await {
                                                        Ok(bytes) => {
                                                            if let Ok(mut file) =
                                                                tokio::fs::File::create(&filepath)
                                                                    .await
                                                                && file
                                                                    .write_all(&bytes)
                                                                    .await
                                                                    .is_ok()
                                                            {
                                                                tracing::debug!(
                                                                    artist = %artist_name,
                                                                    "Downloaded artist image"
                                                                );
                                                                // Update artist record with cover art ID
                                                                if let Err(e) = artist_repo
                                                                    .update_cover_art(
                                                                        artist_id_copy,
                                                                        Some(&cover_art_id),
                                                                    )
                                                                {
                                                                    tracing::warn!(
                                                                        error = %e,
                                                                        "Failed to update artist cover art"
                                                                    );
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            tracing::warn!(error = %e, "Failed to get image bytes");
                                                        }
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                tracing::warn!(error = %e, "Failed to download artist image");
                                            }
                                        }
                                    }
                                }
                            }
                            Ok(None) => {
                                tracing::debug!(artist = %artist_name, "No Last.fm info found");
                            }
                            Err(e) => {
                                tracing::warn!(error = %e, artist = %artist_name, "Failed to fetch Last.fm artist info");
                            }
                        }
                    });
                }
            }
        }

        Ok(response)
    }

    pub(crate) fn get_artist_info_non_id3_with_cache(
        &self,
        artist_id: i32,
    ) -> Result<crate::models::music::ArtistInfoResponse, MusicRepoError> {
        use crate::models::music::{ArtistInfoResponse, ArtistResponse};

        let info2 = self.get_artist_info_with_cache(artist_id)?;

        let similar_artists = info2
            .similar_artists
            .into_iter()
            .map(|a| ArtistResponse {
                id: a.id,
                name: a.name,
                artist_image_url: a.artist_image_url,
                starred: a.starred,
                user_rating: None,
                average_rating: None,
            })
            .collect();

        Ok(ArtistInfoResponse {
            biography: info2.biography,
            musicbrainz_id: info2.musicbrainz_id,
            last_fm_url: info2.last_fm_url,
            small_image_url: info2.small_image_url,
            medium_image_url: info2.medium_image_url,
            large_image_url: info2.large_image_url,
            similar_artists,
        })
    }
}

impl AuthState for DatabaseAuthState {
    fn find_user(&self, username: &str) -> Result<Option<User>, UserRepoError> {
        Self::find_user(self, username)
    }

    fn find_user_by_api_key(&self, api_key: &str) -> Result<Option<User>, UserRepoError> {
        Self::find_user_by_api_key(self, api_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_encoded_password() {
        // "password" in hex is "70617373776f7264"
        let encoded = "enc:70617373776f7264";
        let decoded = AuthParams::decode_password(encoded);
        assert_eq!(decoded.as_deref(), Some("password"));

        // Plain password should be returned as-is
        let plain = "password";
        assert_eq!(
            AuthParams::decode_password(plain).as_deref(),
            Some("password")
        );
    }

    #[test]
    fn test_format_from_param() {
        assert_eq!(Format::from_param(None), Format::Xml);
        assert_eq!(Format::from_param(Some("xml")), Format::Xml);
        assert_eq!(Format::from_param(Some("json")), Format::Json);
        assert_eq!(Format::from_param(Some("jsonp")), Format::Json);
    }

    #[test]
    fn test_params_merge() {
        let query = AuthParams {
            u: "user".into(),
            v: "1.16.1".into(),
            c: "test".into(),
            p: Some("pass".into()),
            ..Default::default()
        };
        let form = AuthParams {
            u: "other".into(),
            v: "1.15.0".into(),
            c: "other_client".into(),
            f: Some("json".into()),
            ..Default::default()
        };

        let merged = query.merge_with(form);

        // Query params should take precedence
        assert_eq!(merged.u, "user");
        assert_eq!(merged.v, "1.16.1");
        assert_eq!(merged.c, "test");
        assert_eq!(merged.p, Some("pass".into()));
        // Form params fill in missing values
        assert_eq!(merged.f, Some("json".into()));
    }

    #[test]
    fn params_merge_preserves_query_auth_fields_and_fills_missing_form_fields() {
        let query = AuthParams {
            u: "query-user".into(),
            t: Some("query-token".into()),
            v: "1.16.1".into(),
            c: "query-client".into(),
            ..Default::default()
        };
        let form = AuthParams {
            u: "form-user".into(),
            p: Some("form-password".into()),
            t: Some("form-token".into()),
            s: Some("form-salt".into()),
            api_key: Some("form-key".into()),
            v: "1.15.0".into(),
            c: "form-client".into(),
            f: Some("json".into()),
        };

        let merged = query.merge_with(form);

        assert_eq!(merged.u, "query-user");
        assert_eq!(merged.t.as_deref(), Some("query-token"));
        assert_eq!(merged.v, "1.16.1");
        assert_eq!(merged.c, "query-client");
        assert_eq!(merged.p.as_deref(), Some("form-password"));
        assert_eq!(merged.s.as_deref(), Some("form-salt"));
        assert_eq!(merged.api_key.as_deref(), Some("form-key"));
        assert_eq!(merged.f.as_deref(), Some("json"));
    }

    #[test]
    fn invalid_hex_password_returns_none() {
        assert_eq!(AuthParams::decode_password("enc:not-hex"), None);
        assert_eq!(AuthParams::decode_password("enc:ff"), None);
    }

    #[test]
    fn test_api_key_detection() {
        let with_api_key = AuthParams {
            api_key: Some("secret".into()),
            v: "1.16.1".into(),
            c: "test".into(),
            ..Default::default()
        };
        assert!(with_api_key.uses_api_key());
        assert!(!with_api_key.uses_user_auth());

        let with_user = AuthParams {
            u: "user".into(),
            p: Some("pass".into()),
            v: "1.16.1".into(),
            c: "test".into(),
            ..Default::default()
        };
        assert!(!with_user.uses_api_key());
        assert!(with_user.uses_user_auth());
    }
}
