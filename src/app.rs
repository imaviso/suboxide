//! Application state and router wiring.

use axum::{Router, extract::FromRef};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::api::services::{MusicLibrary, RemoteSessions, Users};
use crate::api::{SubsonicRouterExt, handlers};
use crate::db::DbPool;
use crate::lastfm::LastFmClient;
use crate::scanner::{ScanState, ScanStateHandle};

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
pub fn create_router(state: AppState) -> Router {
    let rest_routes = Router::new()
        .subsonic_route("/ping", handlers::ping)
        .subsonic_route("/getLicense", handlers::get_license)
        .subsonic_route(
            "/getOpenSubsonicExtensions",
            handlers::get_open_subsonic_extensions,
        )
        .subsonic_route("/tokenInfo", handlers::token_info)
        .subsonic_route("/getBookmarks", handlers::get_bookmarks)
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
        .subsonic_route("/star", handlers::star)
        .subsonic_route("/unstar", handlers::unstar)
        .subsonic_route("/getStarred2", handlers::get_starred2)
        .subsonic_route("/scrobble", handlers::scrobble)
        .subsonic_route("/getNowPlaying", handlers::get_now_playing)
        .subsonic_route("/setRating", handlers::set_rating)
        .subsonic_route("/getPlaylists", handlers::get_playlists)
        .subsonic_route("/getPlaylist", handlers::get_playlist)
        .subsonic_route("/createPlaylist", handlers::create_playlist)
        .subsonic_route("/updatePlaylist", handlers::update_playlist)
        .subsonic_route("/deletePlaylist", handlers::delete_playlist)
        .subsonic_route("/getPlayQueue", handlers::get_play_queue)
        .subsonic_route("/savePlayQueue", handlers::save_play_queue)
        .subsonic_route("/getPlayQueueByIndex", handlers::get_play_queue_by_index)
        .subsonic_route("/savePlayQueueByIndex", handlers::save_play_queue_by_index)
        .subsonic_route("/createRemoteSession", handlers::create_remote_session)
        .subsonic_route("/joinRemoteSession", handlers::join_remote_session)
        .subsonic_route("/getRemoteSession", handlers::get_remote_session)
        .subsonic_route("/closeRemoteSession", handlers::close_remote_session)
        .subsonic_route("/sendRemoteCommand", handlers::send_remote_command)
        .subsonic_route("/getRemoteCommands", handlers::get_remote_commands)
        .subsonic_route("/updateRemoteState", handlers::update_remote_state)
        .subsonic_route("/getRemoteState", handlers::get_remote_state)
        .subsonic_route("/stream", handlers::stream)
        .subsonic_route("/download", handlers::download)
        .subsonic_route("/getCoverArt", handlers::get_cover_art)
        .subsonic_route("/getUser", handlers::get_user)
        .subsonic_route("/getUsers", handlers::get_users)
        .subsonic_route("/deleteUser", handlers::delete_user)
        .subsonic_route("/changePassword", handlers::change_password)
        .subsonic_route("/createUser", handlers::create_user)
        .subsonic_route("/updateUser", handlers::update_user)
        .subsonic_route("/startScan", handlers::start_scan)
        .subsonic_route("/getScanStatus", handlers::get_scan_status);

    Router::new()
        .nest("/rest", rest_routes)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
