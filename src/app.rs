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
    response::{IntoResponse, Response},
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
            rest_routes()
                .layer(middleware::from_fn(post_form_to_query_params))
                .layer(middleware::from_fn(run_request_on_blocking_thread)),
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
        .merge(bookmark_routes())
        .merge(playlist_routes())
        .merge(play_queue_routes())
        .merge(remote_routes())
        .merge(media_routes())
        .merge(radio_routes())
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
        .subsonic_route("/reportPlayback", handlers::report_playback)
        .subsonic_route("/getNowPlaying", handlers::get_now_playing)
        .subsonic_route("/setRating", handlers::set_rating)
}

fn bookmark_routes() -> Router<AppState> {
    Router::new()
        .subsonic_route("/getBookmarks", handlers::get_bookmarks)
        .subsonic_route("/createBookmark", handlers::create_bookmark)
        .subsonic_route("/deleteBookmark", handlers::delete_bookmark)
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
        .subsonic_route("/getAvatar", handlers::get_avatar)
}

fn radio_routes() -> Router<AppState> {
    Router::new()
        .subsonic_route(
            "/getInternetRadioStations",
            handlers::get_internet_radio_stations,
        )
        .subsonic_route(
            "/createInternetRadioStation",
            handlers::create_internet_radio_station,
        )
        .subsonic_route(
            "/updateInternetRadioStation",
            handlers::update_internet_radio_station,
        )
        .subsonic_route(
            "/deleteInternetRadioStation",
            handlers::delete_internet_radio_station,
        )
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

/// Maximum accepted form body size (10MB), matching navidrome.
const MAX_FORM_BODY_BYTES: usize = 10 << 20;

/// Parse `application/x-www-form-urlencoded` POST bodies into query params
/// (`OpenSubsonic` formPost extension).
///
/// Body parameters are placed before query-string parameters so they win
/// when a handler sees duplicate keys, matching navidrome's behavior.
async fn post_form_to_query_params(req: Request<Body>, next: Next) -> Response {
    use axum::http::header;

    if req.method() != axum::http::Method::POST {
        return next.run(req).await;
    }
    let is_form = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|content_type| content_type.starts_with("application/x-www-form-urlencoded"));
    if !is_form {
        return next.run(req).await;
    }

    let (mut parts, body) = req.into_parts();
    let Ok(bytes) = axum::body::to_bytes(body, MAX_FORM_BODY_BYTES).await else {
        return (StatusCode::PAYLOAD_TOO_LARGE, "form body too large").into_response();
    };

    // WHATWG form parsing is lossy: invalid sequences are replaced, never errors.
    let mut pairs: Vec<(String, String)> =
        serde_html_form::from_bytes::<Vec<(String, String)>>(&bytes).unwrap_or_default();
    if let Some(query) = parts.uri.query()
        && let Ok(query_pairs) = serde_html_form::from_str::<Vec<(String, String)>>(query)
    {
        pairs.extend(query_pairs);
    }

    let merged = serde_html_form::to_string(&pairs).unwrap_or_default();
    let path = parts.uri.path().to_owned();
    let path_and_query = if merged.is_empty() {
        path
    } else {
        format!("{path}?{merged}")
    };

    let Ok(path_and_query) = path_and_query.parse::<axum::http::uri::PathAndQuery>() else {
        return (StatusCode::BAD_REQUEST, "invalid request parameters").into_response();
    };
    let mut uri_parts = parts.uri.into_parts();
    uri_parts.path_and_query = Some(path_and_query);
    let Ok(uri) = axum::http::Uri::from_parts(uri_parts) else {
        return (StatusCode::BAD_REQUEST, "invalid request URI").into_response();
    };
    parts.uri = uri;

    next.run(Request::from_parts(parts, Body::empty())).await
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
    use axum::Router;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::{Method, StatusCode, header};
    use axum::middleware;
    use tower::ServiceExt;

    use super::{cors_origins_from_str, post_form_to_query_params};

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

    fn echo_query_router() -> Router {
        async fn echo_query(uri: axum::http::Uri) -> String {
            uri.query().unwrap_or_default().to_string()
        }

        Router::new()
            .route("/ping", axum::routing::get(echo_query).post(echo_query))
            .layer(middleware::from_fn(post_form_to_query_params))
    }

    #[tokio::test]
    async fn post_form_body_is_merged_into_query_string() {
        let response = echo_query_router()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/ping?u=alice")
                    .header(
                        header::CONTENT_TYPE,
                        "application/x-www-form-urlencoded; charset=utf-8",
                    )
                    .body(Body::from("p=secret&id=1&id=2"))
                    .expect("request must build"),
            )
            .await
            .expect("router must respond");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .expect("body must read");
        // Body params come first so they win on duplicate keys; query params
        // and repeated keys are preserved.
        assert_eq!(&*body, b"p=secret&id=1&id=2&u=alice");
    }

    #[tokio::test]
    async fn get_requests_and_non_form_posts_pass_through_untouched() {
        let get_response = echo_query_router()
            .oneshot(
                Request::builder()
                    .uri("/ping?u=alice")
                    .body(Body::empty())
                    .expect("request must build"),
            )
            .await
            .expect("router must respond");
        let body = axum::body::to_bytes(get_response.into_body(), 1024)
            .await
            .expect("body must read");
        assert_eq!(&*body, b"u=alice");

        let json_post = echo_query_router()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/ping?u=alice")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from("{}"))
                    .expect("request must build"),
            )
            .await
            .expect("router must respond");
        let body = axum::body::to_bytes(json_post.into_body(), 1024)
            .await
            .expect("body must read");
        assert_eq!(&*body, b"u=alice");
    }

    #[tokio::test]
    async fn invalid_form_encoding_is_replaced_lossy() {
        let response = echo_query_router()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/ping")
                    .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                    // Invalid UTF-8 in a form value
                    .body(Body::from(vec![b'f', b'=', 0xFF]))
                    .expect("request must build"),
            )
            .await
            .expect("router must respond");

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .expect("body must read");
        let body = String::from_utf8(body.to_vec()).expect("body must be UTF-8");
        assert!(body.starts_with("f="), "unexpected query: {body}");
        assert!(
            body.contains("%EF%BF%BD"),
            "invalid bytes must become replacement chars: {body}"
        );
    }
}
