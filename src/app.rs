//! Application state and router wiring.

use std::time::Duration;

use axum::{
    BoxError, Router,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::FromRef,
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::{self, Next},
    response::Response,
};
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::api::services::{MusicLibrary, RemoteSessions, Users};
use crate::api::{SubsonicRouterExt, handlers};
use crate::db::DbPool;
use crate::lastfm::LastFmClient;
use crate::scanner::{ScanState, ScanStateHandle};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const CORS_ORIGIN_ENV: &str = "SUBOXIDE_CORS_ORIGIN";

/// Cross-origin request configuration.
#[derive(Clone, Debug, Default)]
pub struct CorsConfig {
    allowed_origins: Option<Vec<HeaderValue>>,
}

impl CorsConfig {
    /// Load CORS configuration from environment variables.
    pub fn from_env() -> Result<Self, CorsConfigError> {
        let Some(raw) = std::env::var_os(CORS_ORIGIN_ENV) else {
            return Ok(Self::default());
        };
        let raw = raw.to_string_lossy();
        let origins = cors_origins_from_str(&raw)?;

        Ok(Self {
            allowed_origins: (!origins.is_empty()).then_some(origins),
        })
    }

    fn layer(&self) -> CorsLayer {
        let origin = self
            .allowed_origins
            .clone()
            .map_or_else(AllowOrigin::any, AllowOrigin::list);

        CorsLayer::new()
            .allow_origin(origin)
            .allow_methods(Any)
            .allow_headers(Any)
    }
}

fn cors_origins_from_str(raw: &str) -> Result<Vec<HeaderValue>, CorsConfigError> {
    raw.split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            HeaderValue::from_str(origin).map_err(|source| CorsConfigError {
                origin: origin.to_string(),
                source,
            })
        })
        .collect()
}

/// Error returned when CORS environment configuration is invalid.
#[derive(Debug, thiserror::Error)]
#[error("invalid CORS origin '{origin}' in {CORS_ORIGIN_ENV}: {source}")]
pub struct CorsConfigError {
    origin: String,
    #[source]
    source: axum::http::header::InvalidHeaderValue,
}

impl CorsConfigError {
    /// Return the invalid origin from the CORS configuration.
    #[must_use]
    pub fn origin(&self) -> &str {
        &self.origin
    }
}

/// Application state shared across handlers.
#[derive(Clone, Debug)]
pub struct AppState {
    pool: DbPool,
    scan_state: ScanStateHandle,
    music: MusicLibrary,
    users: Users,
    remote: RemoteSessions,
}

impl AppState {
    /// Create application state.
    #[must_use]
    pub fn new(pool: DbPool, lastfm_client: LastFmClient) -> Self {
        let scan_state = ScanStateHandle::new(ScanState::new());
        let music = MusicLibrary::new(pool.clone(), lastfm_client);
        let users = Users::new(pool.clone());
        let remote = RemoteSessions::new(pool.clone());

        Self {
            pool,
            scan_state,
            music,
            users,
            remote,
        }
    }

    /// Get the shared scan state.
    #[must_use]
    pub fn scan_state(&self) -> ScanStateHandle {
        self.scan_state.clone()
    }
}

impl FromRef<AppState> for MusicLibrary {
    fn from_ref(state: &AppState) -> Self {
        state.music.clone()
    }
}

impl FromRef<AppState> for Users {
    fn from_ref(state: &AppState) -> Self {
        state.users.clone()
    }
}

impl FromRef<AppState> for RemoteSessions {
    fn from_ref(state: &AppState) -> Self {
        state.remote.clone()
    }
}

impl FromRef<AppState> for DbPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for ScanStateHandle {
    fn from_ref(state: &AppState) -> Self {
        state.scan_state.clone()
    }
}

/// Create the main API router.
pub fn create_router(state: AppState, cors_config: &CorsConfig) -> Router {
    Router::new()
        .nest(
            "/rest",
            rest_routes().layer(middleware::from_fn(run_request_on_blocking_thread)),
        )
        .layer(CompressionLayer::new())
        .layer(cors_config.layer())
        .layer(TraceLayer::new_for_http())
        .layer(
            ServiceBuilder::new()
                .layer(HandleErrorLayer::new(handle_middleware_error))
                .layer(tower::timeout::TimeoutLayer::new(REQUEST_TIMEOUT)),
        )
        .with_state(state)
}

fn rest_routes() -> Router<AppState> {
    Router::new()
        .merge(system_routes())
        .merge(browsing_routes())
        .merge(annotation_routes())
        .merge(playlist_routes())
        .merge(play_queue_routes())
        .merge(remote_routes())
        .merge(media_routes())
        .merge(user_routes())
        .merge(scanning_routes())
}

fn system_routes() -> Router<AppState> {
    Router::new()
        .subsonic_route("/ping", handlers::ping)
        .subsonic_route("/getLicense", handlers::get_license)
        .subsonic_route(
            "/getOpenSubsonicExtensions",
            handlers::get_open_subsonic_extensions,
        )
        .subsonic_route("/tokenInfo", handlers::token_info)
        .subsonic_route("/getBookmarks", handlers::get_bookmarks)
}

fn browsing_routes() -> Router<AppState> {
    Router::new()
        .subsonic_route("/getMusicFolders", handlers::get_music_folders)
        .subsonic_route("/getIndexes", handlers::get_indexes)
        .subsonic_route("/getArtists", handlers::get_artists)
        .subsonic_route("/getArtist", handlers::get_artist)
        .subsonic_route("/getAlbum", handlers::get_album)
        .subsonic_route("/getSong", handlers::get_song)
        .subsonic_route("/getAlbumList2", handlers::get_album_list2)
        .subsonic_route("/getGenres", handlers::get_genres)
        .subsonic_route("/search3", handlers::search3)
        .subsonic_route("/getRandomSongs", handlers::get_random_songs)
        .subsonic_route("/getSongsByGenre", handlers::get_songs_by_genre)
        .subsonic_route("/getArtistInfo2", handlers::get_artist_info2)
        .subsonic_route("/getAlbumInfo2", handlers::get_album_info2)
        .subsonic_route("/getSimilarSongs2", handlers::get_similar_songs2)
        .subsonic_route("/getTopSongs", handlers::get_top_songs)
        .subsonic_route("/getMusicDirectory", handlers::get_music_directory)
        .subsonic_route("/getAlbumList", handlers::get_album_list)
        .subsonic_route("/getStarred", handlers::get_starred)
        .subsonic_route("/getArtistInfo", handlers::get_artist_info)
        .subsonic_route("/getAlbumInfo", handlers::get_album_info)
        .subsonic_route("/getSimilarSongs", handlers::get_similar_songs)
        .subsonic_route("/search2", handlers::search2)
        .subsonic_route("/search", handlers::search)
        .subsonic_route("/getLyrics", handlers::get_lyrics)
        .subsonic_route("/getLyricsBySongId", handlers::get_lyrics_by_song_id)
}

fn annotation_routes() -> Router<AppState> {
    Router::new()
        .subsonic_route("/star", handlers::star)
        .subsonic_route("/unstar", handlers::unstar)
        .subsonic_route("/getStarred2", handlers::get_starred2)
        .subsonic_route("/scrobble", handlers::scrobble)
        .subsonic_route("/getNowPlaying", handlers::get_now_playing)
        .subsonic_route("/setRating", handlers::set_rating)
}

fn playlist_routes() -> Router<AppState> {
    Router::new()
        .subsonic_route("/getPlaylists", handlers::get_playlists)
        .subsonic_route("/getPlaylist", handlers::get_playlist)
        .subsonic_route("/createPlaylist", handlers::create_playlist)
        .subsonic_route("/updatePlaylist", handlers::update_playlist)
        .subsonic_route("/deletePlaylist", handlers::delete_playlist)
}

fn play_queue_routes() -> Router<AppState> {
    Router::new()
        .subsonic_route("/getPlayQueue", handlers::get_play_queue)
        .subsonic_route("/savePlayQueue", handlers::save_play_queue)
        .subsonic_route("/getPlayQueueByIndex", handlers::get_play_queue_by_index)
        .subsonic_route("/savePlayQueueByIndex", handlers::save_play_queue_by_index)
}

fn remote_routes() -> Router<AppState> {
    Router::new()
        .subsonic_route("/createRemoteSession", handlers::create_remote_session)
        .subsonic_route("/joinRemoteSession", handlers::join_remote_session)
        .subsonic_route("/getRemoteSession", handlers::get_remote_session)
        .subsonic_route("/closeRemoteSession", handlers::close_remote_session)
        .subsonic_route("/sendRemoteCommand", handlers::send_remote_command)
        .subsonic_route("/getRemoteCommands", handlers::get_remote_commands)
        .subsonic_route("/updateRemoteState", handlers::update_remote_state)
        .subsonic_route("/getRemoteState", handlers::get_remote_state)
}

fn media_routes() -> Router<AppState> {
    Router::new()
        .subsonic_route("/stream", handlers::stream)
        .subsonic_route("/download", handlers::download)
        .subsonic_route("/getCoverArt", handlers::get_cover_art)
}

fn user_routes() -> Router<AppState> {
    Router::new()
        .subsonic_route("/getUser", handlers::get_user)
        .subsonic_route("/getUsers", handlers::get_users)
        .subsonic_route("/deleteUser", handlers::delete_user)
        .subsonic_route("/changePassword", handlers::change_password)
        .subsonic_route("/createUser", handlers::create_user)
        .subsonic_route("/updateUser", handlers::update_user)
}

fn scanning_routes() -> Router<AppState> {
    Router::new()
        .subsonic_route("/startScan", handlers::start_scan)
        .subsonic_route("/getScanStatus", handlers::get_scan_status)
}

async fn run_request_on_blocking_thread(req: Request<Body>, next: Next) -> Response {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return next.run(req).await;
    };

    if matches!(
        handle.runtime_flavor(),
        tokio::runtime::RuntimeFlavor::MultiThread
    ) {
        tokio::task::block_in_place(|| handle.block_on(next.run(req)))
    } else {
        next.run(req).await
    }
}

async fn handle_middleware_error(error: BoxError) -> (StatusCode, &'static str) {
    if error.is::<tower::timeout::error::Elapsed>() {
        tracing::warn!(name = "http.request.timeout", "request timed out");
        return (StatusCode::REQUEST_TIMEOUT, "request timed out");
    }

    tracing::error!(
        name = "http.middleware.failed",
        error = %error,
        "middleware failed"
    );
    (StatusCode::INTERNAL_SERVER_ERROR, "internal server error")
}

#[cfg(test)]
mod tests {
    use super::cors_origins_from_str;

    #[test]
    fn cors_origins_from_str_trims_and_ignores_empty_segments() {
        let origins = cors_origins_from_str(" https://a.example, ,https://b.example ")
            .expect("valid origins should parse");

        assert_eq!(origins.len(), 2);
        assert_eq!(origins[0], "https://a.example");
        assert_eq!(origins[1], "https://b.example");
    }

    #[test]
    fn cors_origins_from_str_rejects_invalid_header_values() {
        let error = cors_origins_from_str("https://good.example,https://bad\n.example")
            .expect_err("newline makes header value invalid");

        assert_eq!(error.origin(), "https://bad\n.example");
    }
}
