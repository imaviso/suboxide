//! Subsonic API response types and serialization.
//!
//! Supports both XML and JSON response formats as per the Subsonic API spec.
//! The format is determined by the `f` query parameter (xml, json, jsonp).

use axum::{
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;

use super::error::ApiError;
use crate::models::music::{
    AlbumInfoResponse, AlbumList2Response, AlbumListResponse, AlbumWithSongsID3Response,
    ArtistInfo2Response, ArtistInfoResponse, ArtistWithAlbumsID3Response, ArtistsID3Response,
    ChildResponse, DirectoryResponse, GenresResponse, IndexesResponse, LyricsListResponse,
    LyricsResponse, MusicFolderResponse, NowPlayingResponse, PlayQueueByIndexResponse,
    PlayQueueResponse, PlaylistWithSongsResponse, PlaylistsResponse, RandomSongsResponse,
    SearchResult2Response, SearchResult3Response, SearchResultResponse, SimilarSongs2Response,
    SimilarSongsResponse, SongsByGenreResponse, Starred2Response, StarredResponse,
    TokenInfoResponse, TopSongsResponse,
};
use crate::models::user::{UserResponse, UsersResponse};

/// The current Subsonic API version we're compatible with.
pub const API_VERSION: &str = "1.16.1";

/// Server name reported in responses.
pub const SERVER_NAME: &str = "subsonic-rs";

/// Server version from Cargo.toml.
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Response format requested by the client.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Format {
    #[default]
    Xml,
    Json,
}

impl Format {
    #[must_use]
    pub fn from_param(f: Option<&str>) -> Self {
        match f {
            Some("json" | "jsonp") => Self::Json,
            _ => Self::Xml,
        }
    }
}

/// Response status values.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ResponseStatus {
    Ok,
    Failed,
}

// ============================================================================
// Scan Status Types
// ============================================================================

/// Enhanced scan status data for API responses.
#[derive(Debug, Clone, Default)]
pub struct ScanStatusData {
    /// Whether a scan is currently in progress.
    pub scanning: bool,
    /// Number of items scanned so far.
    pub count: u64,
    /// Total number of items to scan (0 if unknown).
    pub total: u64,
    /// Current scan phase (idle, discovering, processing, cleaning).
    pub phase: String,
    /// Current folder being scanned (if any).
    pub folder: Option<String>,
}

// ============================================================================
// OpenSubsonic Extension Types
// ============================================================================

/// Represents a supported `OpenSubsonic` API extension.
#[derive(Debug, Clone, Serialize)]
pub struct OpenSubsonicExtension {
    /// The name of the extension.
    pub name: String,
    /// The list of supported versions of this extension.
    pub versions: Vec<i32>,
}

impl OpenSubsonicExtension {
    pub fn new(name: impl Into<String>, versions: Vec<i32>) -> Self {
        Self {
            name: name.into(),
            versions,
        }
    }
}

/// Returns the list of `OpenSubsonic` extensions supported by this server.
#[must_use]
pub fn supported_extensions() -> Vec<OpenSubsonicExtension> {
    vec![
        OpenSubsonicExtension::new("formPost", vec![1]),
        OpenSubsonicExtension::new("apiKeyAuthentication", vec![1]),
        OpenSubsonicExtension::new("songLyrics", vec![1]),
    ]
}

// ============================================================================
// XML Response Types (use @attribute naming)
// ============================================================================

mod xml {
    use super::{API_VERSION, ResponseStatus, SERVER_NAME, SERVER_VERSION, Serialize};

    // Note: quick_xml doesn't support #[serde(flatten)], so we need to include
    // all base attributes directly in each response struct.

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct EmptyResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
    }

    impl EmptyResponse {
        pub const fn ok() -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct ErrorDetail {
        #[serde(rename = "@code")]
        pub code: u32,
        #[serde(rename = "@message")]
        pub message: String,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct ErrorResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        pub error: ErrorDetail,
    }

    impl ErrorResponse {
        pub const fn new(code: u32, message: String) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Failed,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                error: ErrorDetail { code, message },
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct License {
        #[serde(rename = "@valid")]
        pub valid: bool,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct LicenseResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        pub license: License,
    }

    impl LicenseResponse {
        pub const fn ok() -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                license: License { valid: true },
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct OpenSubsonicExtensionXml {
        #[serde(rename = "@name")]
        pub name: String,
        #[serde(rename = "version")]
        pub versions: Vec<i32>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct OpenSubsonicExtensionsResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "openSubsonicExtensions")]
        pub extensions: Vec<OpenSubsonicExtensionXml>,
    }

    impl OpenSubsonicExtensionsResponse {
        pub const fn new(extensions: Vec<OpenSubsonicExtensionXml>) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                extensions,
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MusicFolders {
        #[serde(rename = "musicFolder")]
        pub folders: Vec<super::MusicFolderResponse>,
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct MusicFoldersResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "musicFolders")]
        pub music_folders: MusicFolders,
    }

    impl MusicFoldersResponse {
        pub const fn new(folders: Vec<super::MusicFolderResponse>) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                music_folders: MusicFolders { folders },
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct IndexesResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "indexes")]
        pub indexes: super::IndexesResponse,
    }

    impl IndexesResponse {
        pub const fn new(indexes: super::IndexesResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                indexes,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct ArtistsResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "artists")]
        pub artists: super::ArtistsID3Response,
    }

    impl ArtistsResponse {
        pub const fn new(artists: super::ArtistsID3Response) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                artists,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct AlbumResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "album")]
        pub album: super::AlbumWithSongsID3Response,
    }

    impl AlbumResponse {
        pub const fn new(album: super::AlbumWithSongsID3Response) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                album,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct ArtistResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "artist")]
        pub artist: super::ArtistWithAlbumsID3Response,
    }

    impl ArtistResponse {
        pub const fn new(artist: super::ArtistWithAlbumsID3Response) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                artist,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct SongResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "song")]
        pub song: super::ChildResponse,
    }

    impl SongResponse {
        pub const fn new(song: super::ChildResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                song,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct AlbumList2Response {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "albumList2")]
        pub album_list2: super::AlbumList2Response,
    }

    impl AlbumList2Response {
        pub const fn new(album_list2: super::AlbumList2Response) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                album_list2,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct GenresResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "genres")]
        pub genres: super::GenresResponse,
    }

    impl GenresResponse {
        pub const fn new(genres: super::GenresResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                genres,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct SearchResult3Response {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "searchResult3")]
        pub search_result3: super::SearchResult3Response,
    }

    impl SearchResult3Response {
        pub const fn new(search_result3: super::SearchResult3Response) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                search_result3,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct Starred2Response {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "starred2")]
        pub starred2: super::Starred2Response,
    }

    impl Starred2Response {
        pub const fn new(starred2: super::Starred2Response) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                starred2,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct NowPlayingResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "nowPlaying")]
        pub now_playing: super::NowPlayingResponse,
    }

    impl NowPlayingResponse {
        pub const fn new(now_playing: super::NowPlayingResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                now_playing,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct RandomSongsResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "randomSongs")]
        pub random_songs: super::RandomSongsResponse,
    }

    impl RandomSongsResponse {
        pub const fn new(random_songs: super::RandomSongsResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                random_songs,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct SongsByGenreResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "songsByGenre")]
        pub songs_by_genre: super::SongsByGenreResponse,
    }

    impl SongsByGenreResponse {
        pub const fn new(songs_by_genre: super::SongsByGenreResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                songs_by_genre,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct PlaylistsResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "playlists")]
        pub playlists: super::PlaylistsResponse,
    }

    impl PlaylistsResponse {
        pub const fn new(playlists: super::PlaylistsResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                playlists,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct PlaylistResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "playlist")]
        pub playlist: super::PlaylistWithSongsResponse,
    }

    impl PlaylistResponse {
        pub const fn new(playlist: super::PlaylistWithSongsResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                playlist,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct PlayQueueResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "playQueue")]
        pub play_queue: super::PlayQueueResponse,
    }

    impl PlayQueueResponse {
        pub const fn new(play_queue: super::PlayQueueResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                play_queue,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct PlayQueueByIndexResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "playQueueByIndex")]
        pub play_queue_by_index: super::PlayQueueByIndexResponse,
    }

    impl PlayQueueByIndexResponse {
        pub const fn new(play_queue_by_index: super::PlayQueueByIndexResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                play_queue_by_index,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct TokenInfoResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "tokenInfo")]
        pub token_info: super::TokenInfoResponse,
    }

    impl TokenInfoResponse {
        pub const fn new(token_info: super::TokenInfoResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                token_info,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct UserResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "user")]
        pub user: super::UserResponse,
    }

    impl UserResponse {
        pub const fn new(user: super::UserResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                user,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct UsersResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "users")]
        pub users: super::UsersResponse,
    }

    impl UsersResponse {
        pub const fn new(users: super::UsersResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                users,
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct ScanStatus {
        #[serde(rename = "@scanning")]
        pub scanning: bool,
        #[serde(rename = "@count")]
        pub count: u64,
        #[serde(rename = "@total", skip_serializing_if = "Option::is_none")]
        pub total: Option<u64>,
        #[serde(rename = "@phase", skip_serializing_if = "Option::is_none")]
        pub phase: Option<String>,
        #[serde(rename = "@folder", skip_serializing_if = "Option::is_none")]
        pub folder: Option<String>,
    }

    impl ScanStatus {
        pub fn from_data(data: &super::ScanStatusData) -> Self {
            Self {
                scanning: data.scanning,
                count: data.count,
                total: if data.total > 0 {
                    Some(data.total)
                } else {
                    None
                },
                phase: if data.scanning {
                    Some(data.phase.clone())
                } else {
                    None
                },
                folder: data.folder.clone(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct ScanStatusResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "scanStatus")]
        pub scan_status: ScanStatus,
    }

    impl ScanStatusResponse {
        pub fn from_data(data: &super::ScanStatusData) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                scan_status: ScanStatus::from_data(data),
            }
        }
    }

    /// Empty bookmarks response for XML format.
    #[derive(Debug, Serialize)]
    pub struct Bookmarks {
        // Empty - no bookmarks implemented yet
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct BookmarksResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "bookmarks")]
        pub bookmarks: Bookmarks,
    }

    impl BookmarksResponse {
        pub const fn new() -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                bookmarks: Bookmarks {},
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct ArtistInfo2Response {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "artistInfo2")]
        pub artist_info2: super::ArtistInfo2Response,
    }

    impl ArtistInfo2Response {
        pub const fn new(artist_info2: super::ArtistInfo2Response) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                artist_info2,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct AlbumInfoResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "albumInfo")]
        pub album_info: super::AlbumInfoResponse,
    }

    impl AlbumInfoResponse {
        pub const fn new(album_info: super::AlbumInfoResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                album_info,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct SimilarSongs2Response {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "similarSongs2")]
        pub similar_songs2: super::SimilarSongs2Response,
    }

    impl SimilarSongs2Response {
        pub const fn new(similar_songs2: super::SimilarSongs2Response) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                similar_songs2,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct TopSongsResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "topSongs")]
        pub top_songs: super::TopSongsResponse,
    }

    impl TopSongsResponse {
        pub const fn new(top_songs: super::TopSongsResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                top_songs,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct LyricsResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "lyrics")]
        pub lyrics: super::LyricsResponse,
    }

    impl LyricsResponse {
        pub const fn new(lyrics: super::LyricsResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                lyrics,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct LyricsListResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "lyricsList")]
        pub lyrics_list: super::LyricsListResponse,
    }

    impl LyricsListResponse {
        pub const fn new(lyrics_list: super::LyricsListResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                lyrics_list,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct DirectoryResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "directory")]
        pub directory: super::DirectoryResponse,
    }

    impl DirectoryResponse {
        pub const fn new(directory: super::DirectoryResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                directory,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct AlbumListResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "albumList")]
        pub album_list: super::AlbumListResponse,
    }

    impl AlbumListResponse {
        pub const fn new(album_list: super::AlbumListResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                album_list,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct StarredResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "starred")]
        pub starred: super::StarredResponse,
    }

    impl StarredResponse {
        pub const fn new(starred: super::StarredResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                starred,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct SearchResult2Response {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "searchResult2")]
        pub search_result2: super::SearchResult2Response,
    }

    impl SearchResult2Response {
        pub const fn new(search_result2: super::SearchResult2Response) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                search_result2,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct SearchResultResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "searchResult")]
        pub search_result: super::SearchResultResponse,
    }

    impl SearchResultResponse {
        pub const fn new(search_result: super::SearchResultResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                search_result,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct ArtistInfoResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "artistInfo")]
        pub artist_info: super::ArtistInfoResponse,
    }

    impl ArtistInfoResponse {
        pub const fn new(artist_info: super::ArtistInfoResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                artist_info,
            }
        }
    }

    #[derive(Debug, Serialize)]
    #[serde(rename = "subsonic-response")]
    pub struct SimilarSongsResponse {
        #[serde(rename = "@xmlns")]
        pub xmlns: &'static str,
        #[serde(rename = "@status")]
        pub status: ResponseStatus,
        #[serde(rename = "@version")]
        pub version: &'static str,
        #[serde(rename = "@type")]
        pub server_type: &'static str,
        #[serde(rename = "@serverVersion")]
        pub server_version: &'static str,
        #[serde(rename = "@openSubsonic")]
        pub open_subsonic: bool,
        #[serde(rename = "similarSongs")]
        pub similar_songs: super::SimilarSongsResponse,
    }

    impl SimilarSongsResponse {
        pub const fn new(similar_songs: super::SimilarSongsResponse) -> Self {
            Self {
                xmlns: "http://subsonic.org/restapi",
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                similar_songs,
            }
        }
    }
}

// ============================================================================
// JSON Response Types (use camelCase naming, wrapped in subsonic-response)
// ============================================================================

mod json {
    use super::{
        API_VERSION, OpenSubsonicExtension, ResponseStatus, SERVER_NAME, SERVER_VERSION, Serialize,
    };

    #[derive(Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct SubsonicResponse {
        pub status: ResponseStatus,
        pub version: &'static str,
        #[serde(rename = "type")]
        pub server_type: &'static str,
        pub server_version: &'static str,
        pub open_subsonic: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<ErrorDetail>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub license: Option<License>,
        #[serde(
            skip_serializing_if = "Option::is_none",
            rename = "openSubsonicExtensions"
        )]
        pub open_subsonic_extensions: Option<Vec<OpenSubsonicExtension>>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "musicFolders")]
        pub music_folders: Option<MusicFoldersJson>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub indexes: Option<super::IndexesResponse>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub artists: Option<super::ArtistsID3Response>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub album: Option<super::AlbumWithSongsID3Response>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub artist: Option<super::ArtistWithAlbumsID3Response>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub song: Option<super::ChildResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "albumList2")]
        pub album_list2: Option<super::AlbumList2Response>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub genres: Option<super::GenresResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "searchResult3")]
        pub search_result3: Option<super::SearchResult3Response>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub starred2: Option<super::Starred2Response>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "nowPlaying")]
        pub now_playing: Option<super::NowPlayingResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "randomSongs")]
        pub random_songs: Option<super::RandomSongsResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "songsByGenre")]
        pub songs_by_genre: Option<super::SongsByGenreResponse>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub playlists: Option<super::PlaylistsResponse>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub playlist: Option<super::PlaylistWithSongsResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "playQueue")]
        pub play_queue: Option<super::PlayQueueResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "playQueueByIndex")]
        pub play_queue_by_index: Option<super::PlayQueueByIndexResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "tokenInfo")]
        pub token_info: Option<super::TokenInfoResponse>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub user: Option<super::UserResponse>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub users: Option<super::UsersResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "scanStatus")]
        pub scan_status: Option<ScanStatusJson>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub bookmarks: Option<BookmarksJson>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "artistInfo2")]
        pub artist_info2: Option<super::ArtistInfo2Response>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "albumInfo")]
        pub album_info: Option<super::AlbumInfoResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "similarSongs2")]
        pub similar_songs2: Option<super::SimilarSongs2Response>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "topSongs")]
        pub top_songs: Option<super::TopSongsResponse>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub lyrics: Option<super::LyricsResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "lyricsList")]
        pub lyrics_list: Option<super::LyricsListResponse>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub directory: Option<super::DirectoryResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "albumList")]
        pub album_list: Option<super::AlbumListResponse>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub starred: Option<super::StarredResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "searchResult2")]
        pub search_result2: Option<super::SearchResult2Response>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "searchResult")]
        pub search_result: Option<super::SearchResultResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "artistInfo")]
        pub artist_info: Option<super::ArtistInfoResponse>,
        #[serde(skip_serializing_if = "Option::is_none", rename = "similarSongs")]
        pub similar_songs: Option<super::SimilarSongsResponse>,
    }

    #[derive(Debug, Serialize)]
    pub struct ScanStatusJson {
        pub scanning: bool,
        pub count: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub total: Option<u64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub phase: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub folder: Option<String>,
    }

    impl ScanStatusJson {
        pub fn from_data(data: &super::ScanStatusData) -> Self {
            Self {
                scanning: data.scanning,
                count: data.count,
                total: if data.total > 0 {
                    Some(data.total)
                } else {
                    None
                },
                phase: if data.scanning {
                    Some(data.phase.clone())
                } else {
                    None
                },
                folder: data.folder.clone(),
            }
        }
    }

    /// Empty bookmarks response for JSON format.
    #[derive(Debug, Serialize)]
    pub struct BookmarksJson {
        // Empty - no bookmarks implemented yet
    }

    #[derive(Debug, Serialize)]
    pub struct MusicFoldersJson {
        #[serde(rename = "musicFolder")]
        pub folders: Vec<super::MusicFolderResponse>,
    }

    #[derive(Debug, Serialize)]
    pub struct ErrorDetail {
        pub code: u32,
        pub message: String,
    }

    #[derive(Debug, Serialize)]
    pub struct License {
        pub valid: bool,
    }

    #[derive(Debug, Serialize)]
    pub struct JsonWrapper {
        #[serde(rename = "subsonic-response")]
        pub subsonic_response: SubsonicResponse,
    }

    impl SubsonicResponse {
        pub const fn ok() -> Self {
            Self {
                status: ResponseStatus::Ok,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                error: None,
                license: None,
                open_subsonic_extensions: None,
                music_folders: None,
                indexes: None,
                artists: None,
                album: None,
                artist: None,
                song: None,
                album_list2: None,
                genres: None,
                search_result3: None,
                starred2: None,
                now_playing: None,
                random_songs: None,
                songs_by_genre: None,
                playlists: None,
                playlist: None,
                play_queue: None,
                play_queue_by_index: None,
                token_info: None,
                user: None,
                users: None,
                scan_status: None,
                bookmarks: None,
                artist_info2: None,
                album_info: None,
                similar_songs2: None,
                top_songs: None,
                lyrics: None,
                lyrics_list: None,
                directory: None,
                album_list: None,
                starred: None,
                search_result2: None,
                search_result: None,
                artist_info: None,
                similar_songs: None,
            }
        }

        pub const fn error(code: u32, message: String) -> Self {
            Self {
                status: ResponseStatus::Failed,
                version: API_VERSION,
                server_type: SERVER_NAME,
                server_version: SERVER_VERSION,
                open_subsonic: true,
                error: Some(ErrorDetail { code, message }),
                license: None,
                open_subsonic_extensions: None,
                music_folders: None,
                indexes: None,
                artists: None,
                album: None,
                artist: None,
                song: None,
                album_list2: None,
                genres: None,
                search_result3: None,
                starred2: None,
                now_playing: None,
                random_songs: None,
                songs_by_genre: None,
                playlists: None,
                playlist: None,
                play_queue: None,
                play_queue_by_index: None,
                token_info: None,
                user: None,
                users: None,
                scan_status: None,
                bookmarks: None,
                artist_info2: None,
                album_info: None,
                similar_songs2: None,
                top_songs: None,
                lyrics: None,
                lyrics_list: None,
                directory: None,
                album_list: None,
                starred: None,
                search_result2: None,
                search_result: None,
                artist_info: None,
                similar_songs: None,
            }
        }

        pub const fn with_license(mut self) -> Self {
            self.license = Some(License { valid: true });
            self
        }

        pub fn with_extensions(mut self, extensions: Vec<OpenSubsonicExtension>) -> Self {
            self.open_subsonic_extensions = Some(extensions);
            self
        }

        pub fn with_music_folders(mut self, folders: Vec<super::MusicFolderResponse>) -> Self {
            self.music_folders = Some(MusicFoldersJson { folders });
            self
        }

        pub fn with_indexes(mut self, indexes: super::IndexesResponse) -> Self {
            self.indexes = Some(indexes);
            self
        }

        pub fn with_artists(mut self, artists: super::ArtistsID3Response) -> Self {
            self.artists = Some(artists);
            self
        }

        pub fn with_album(mut self, album: super::AlbumWithSongsID3Response) -> Self {
            self.album = Some(album);
            self
        }

        pub fn with_artist(mut self, artist: super::ArtistWithAlbumsID3Response) -> Self {
            self.artist = Some(artist);
            self
        }

        pub fn with_song(mut self, song: super::ChildResponse) -> Self {
            self.song = Some(song);
            self
        }

        pub fn with_album_list2(mut self, album_list2: super::AlbumList2Response) -> Self {
            self.album_list2 = Some(album_list2);
            self
        }

        pub fn with_genres(mut self, genres: super::GenresResponse) -> Self {
            self.genres = Some(genres);
            self
        }

        pub fn with_search_result3(mut self, search_result3: super::SearchResult3Response) -> Self {
            self.search_result3 = Some(search_result3);
            self
        }

        pub fn with_starred2(mut self, starred2: super::Starred2Response) -> Self {
            self.starred2 = Some(starred2);
            self
        }

        pub fn with_now_playing(mut self, now_playing: super::NowPlayingResponse) -> Self {
            self.now_playing = Some(now_playing);
            self
        }

        pub fn with_random_songs(mut self, random_songs: super::RandomSongsResponse) -> Self {
            self.random_songs = Some(random_songs);
            self
        }

        pub fn with_songs_by_genre(mut self, songs_by_genre: super::SongsByGenreResponse) -> Self {
            self.songs_by_genre = Some(songs_by_genre);
            self
        }

        pub fn with_playlists(mut self, playlists: super::PlaylistsResponse) -> Self {
            self.playlists = Some(playlists);
            self
        }

        pub fn with_playlist(mut self, playlist: super::PlaylistWithSongsResponse) -> Self {
            self.playlist = Some(playlist);
            self
        }

        pub fn with_play_queue(mut self, play_queue: super::PlayQueueResponse) -> Self {
            self.play_queue = Some(play_queue);
            self
        }

        pub fn with_play_queue_by_index(
            mut self,
            play_queue_by_index: super::PlayQueueByIndexResponse,
        ) -> Self {
            self.play_queue_by_index = Some(play_queue_by_index);
            self
        }

        pub fn with_token_info(mut self, token_info: super::TokenInfoResponse) -> Self {
            self.token_info = Some(token_info);
            self
        }

        pub fn with_user(mut self, user: super::UserResponse) -> Self {
            self.user = Some(user);
            self
        }

        pub fn with_users(mut self, users: super::UsersResponse) -> Self {
            self.users = Some(users);
            self
        }

        pub fn with_scan_status(mut self, data: &super::ScanStatusData) -> Self {
            self.scan_status = Some(ScanStatusJson::from_data(data));
            self
        }

        pub const fn with_bookmarks(mut self) -> Self {
            self.bookmarks = Some(BookmarksJson {});
            self
        }

        pub fn with_artist_info2(mut self, artist_info2: super::ArtistInfo2Response) -> Self {
            self.artist_info2 = Some(artist_info2);
            self
        }

        pub fn with_album_info(mut self, album_info: super::AlbumInfoResponse) -> Self {
            self.album_info = Some(album_info);
            self
        }

        pub fn with_similar_songs2(mut self, similar_songs2: super::SimilarSongs2Response) -> Self {
            self.similar_songs2 = Some(similar_songs2);
            self
        }

        pub fn with_top_songs(mut self, top_songs: super::TopSongsResponse) -> Self {
            self.top_songs = Some(top_songs);
            self
        }

        pub fn with_lyrics(mut self, lyrics: super::LyricsResponse) -> Self {
            self.lyrics = Some(lyrics);
            self
        }

        pub fn with_lyrics_list(mut self, lyrics_list: super::LyricsListResponse) -> Self {
            self.lyrics_list = Some(lyrics_list);
            self
        }

        pub fn with_directory(mut self, directory: super::DirectoryResponse) -> Self {
            self.directory = Some(directory);
            self
        }

        pub fn with_album_list(mut self, album_list: super::AlbumListResponse) -> Self {
            self.album_list = Some(album_list);
            self
        }

        pub fn with_starred(mut self, starred: super::StarredResponse) -> Self {
            self.starred = Some(starred);
            self
        }

        pub fn with_search_result2(mut self, search_result2: super::SearchResult2Response) -> Self {
            self.search_result2 = Some(search_result2);
            self
        }

        pub fn with_search_result(mut self, search_result: super::SearchResultResponse) -> Self {
            self.search_result = Some(search_result);
            self
        }

        pub fn with_artist_info(mut self, artist_info: super::ArtistInfoResponse) -> Self {
            self.artist_info = Some(artist_info);
            self
        }

        pub fn with_similar_songs(mut self, similar_songs: super::SimilarSongsResponse) -> Self {
            self.similar_songs = Some(similar_songs);
            self
        }

        pub const fn wrap(self) -> JsonWrapper {
            JsonWrapper {
                subsonic_response: self,
            }
        }
    }
}

// ============================================================================
// Format-aware Response Types
// ============================================================================

/// A Subsonic API response that can be serialized to XML or JSON.
pub struct SubsonicResponse {
    format: Format,
    kind: ResponseKind,
}

#[allow(clippy::large_enum_variant)]
enum ResponseKind {
    Empty,
    License,
    Error { code: u32, message: String },
    OpenSubsonicExtensions(Vec<OpenSubsonicExtension>),
    MusicFolders(Vec<MusicFolderResponse>),
    Indexes(IndexesResponse),
    Artists(ArtistsID3Response),
    Album(AlbumWithSongsID3Response),
    Artist(ArtistWithAlbumsID3Response),
    Song(ChildResponse),
    AlbumList2(AlbumList2Response),
    Genres(GenresResponse),
    SearchResult3(SearchResult3Response),
    Starred2(Starred2Response),
    NowPlaying(NowPlayingResponse),
    RandomSongs(RandomSongsResponse),
    SongsByGenre(SongsByGenreResponse),
    Playlists(PlaylistsResponse),
    Playlist(PlaylistWithSongsResponse),
    PlayQueue(PlayQueueResponse),
    PlayQueueByIndex(PlayQueueByIndexResponse),
    TokenInfo(TokenInfoResponse),
    User(UserResponse),
    Users(UsersResponse),
    ScanStatus(ScanStatusData),
    Bookmarks,
    ArtistInfo2(ArtistInfo2Response),
    AlbumInfo(AlbumInfoResponse),
    SimilarSongs2(SimilarSongs2Response),
    TopSongs(TopSongsResponse),
    Lyrics(LyricsResponse),
    LyricsList(LyricsListResponse),
    // Non-ID3 endpoints
    Directory(DirectoryResponse),
    AlbumList(AlbumListResponse),
    Starred(StarredResponse),
    SearchResult2(SearchResult2Response),
    SearchResult(SearchResultResponse),
    ArtistInfo(ArtistInfoResponse),
    SimilarSongs(SimilarSongsResponse),
}

impl SubsonicResponse {
    #[must_use]
    pub const fn empty(format: Format) -> Self {
        Self {
            format,
            kind: ResponseKind::Empty,
        }
    }

    #[must_use]
    pub const fn license(format: Format) -> Self {
        Self {
            format,
            kind: ResponseKind::License,
        }
    }

    #[must_use]
    pub fn error(format: Format, error: &ApiError) -> Self {
        Self {
            format,
            kind: ResponseKind::Error {
                code: error.code(),
                message: error.message(),
            },
        }
    }

    #[must_use]
    pub const fn open_subsonic_extensions(
        format: Format,
        extensions: Vec<OpenSubsonicExtension>,
    ) -> Self {
        Self {
            format,
            kind: ResponseKind::OpenSubsonicExtensions(extensions),
        }
    }

    #[must_use]
    pub const fn music_folders(format: Format, folders: Vec<MusicFolderResponse>) -> Self {
        Self {
            format,
            kind: ResponseKind::MusicFolders(folders),
        }
    }

    #[must_use]
    pub const fn indexes(format: Format, indexes: IndexesResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::Indexes(indexes),
        }
    }

    #[must_use]
    pub const fn artists(format: Format, artists: ArtistsID3Response) -> Self {
        Self {
            format,
            kind: ResponseKind::Artists(artists),
        }
    }

    #[must_use]
    pub const fn album(format: Format, album: AlbumWithSongsID3Response) -> Self {
        Self {
            format,
            kind: ResponseKind::Album(album),
        }
    }

    #[must_use]
    pub const fn artist(format: Format, artist: ArtistWithAlbumsID3Response) -> Self {
        Self {
            format,
            kind: ResponseKind::Artist(artist),
        }
    }

    #[must_use]
    pub const fn song(format: Format, song: ChildResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::Song(song),
        }
    }

    #[must_use]
    pub const fn album_list2(format: Format, album_list2: AlbumList2Response) -> Self {
        Self {
            format,
            kind: ResponseKind::AlbumList2(album_list2),
        }
    }

    #[must_use]
    pub const fn genres(format: Format, genres: GenresResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::Genres(genres),
        }
    }

    #[must_use]
    pub const fn search_result3(format: Format, search_result3: SearchResult3Response) -> Self {
        Self {
            format,
            kind: ResponseKind::SearchResult3(search_result3),
        }
    }

    #[must_use]
    pub const fn starred2(format: Format, starred2: Starred2Response) -> Self {
        Self {
            format,
            kind: ResponseKind::Starred2(starred2),
        }
    }

    #[must_use]
    pub const fn now_playing(format: Format, now_playing: NowPlayingResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::NowPlaying(now_playing),
        }
    }

    #[must_use]
    pub const fn random_songs(format: Format, random_songs: RandomSongsResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::RandomSongs(random_songs),
        }
    }

    #[must_use]
    pub const fn songs_by_genre(format: Format, songs_by_genre: SongsByGenreResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::SongsByGenre(songs_by_genre),
        }
    }

    #[must_use]
    pub const fn playlists(format: Format, playlists: PlaylistsResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::Playlists(playlists),
        }
    }

    #[must_use]
    pub const fn playlist(format: Format, playlist: PlaylistWithSongsResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::Playlist(playlist),
        }
    }

    #[must_use]
    pub const fn play_queue(format: Format, play_queue: PlayQueueResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::PlayQueue(play_queue),
        }
    }

    #[must_use]
    pub const fn play_queue_by_index(
        format: Format,
        play_queue_by_index: PlayQueueByIndexResponse,
    ) -> Self {
        Self {
            format,
            kind: ResponseKind::PlayQueueByIndex(play_queue_by_index),
        }
    }

    #[must_use]
    pub const fn token_info(format: Format, token_info: TokenInfoResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::TokenInfo(token_info),
        }
    }

    #[must_use]
    pub const fn user(format: Format, user: UserResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::User(user),
        }
    }

    #[must_use]
    pub const fn users(format: Format, users: UsersResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::Users(users),
        }
    }

    #[must_use]
    pub const fn scan_status(format: Format, data: ScanStatusData) -> Self {
        Self {
            format,
            kind: ResponseKind::ScanStatus(data),
        }
    }

    #[must_use]
    pub const fn bookmarks(format: Format) -> Self {
        Self {
            format,
            kind: ResponseKind::Bookmarks,
        }
    }

    #[must_use]
    pub const fn artist_info2(format: Format, artist_info2: ArtistInfo2Response) -> Self {
        Self {
            format,
            kind: ResponseKind::ArtistInfo2(artist_info2),
        }
    }

    #[must_use]
    pub const fn album_info(format: Format, album_info: AlbumInfoResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::AlbumInfo(album_info),
        }
    }

    #[must_use]
    pub const fn similar_songs2(format: Format, similar_songs2: SimilarSongs2Response) -> Self {
        Self {
            format,
            kind: ResponseKind::SimilarSongs2(similar_songs2),
        }
    }

    #[must_use]
    pub const fn top_songs(format: Format, top_songs: TopSongsResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::TopSongs(top_songs),
        }
    }

    #[must_use]
    pub const fn lyrics(format: Format, lyrics: LyricsResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::Lyrics(lyrics),
        }
    }

    #[must_use]
    pub const fn lyrics_list(format: Format, lyrics_list: LyricsListResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::LyricsList(lyrics_list),
        }
    }

    #[must_use]
    pub const fn directory(format: Format, directory: DirectoryResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::Directory(directory),
        }
    }

    #[must_use]
    pub const fn album_list(format: Format, album_list: AlbumListResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::AlbumList(album_list),
        }
    }

    #[must_use]
    pub const fn starred(format: Format, starred: StarredResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::Starred(starred),
        }
    }

    #[must_use]
    pub const fn search_result2(format: Format, search_result2: SearchResult2Response) -> Self {
        Self {
            format,
            kind: ResponseKind::SearchResult2(search_result2),
        }
    }

    #[must_use]
    pub const fn search_result(format: Format, search_result: SearchResultResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::SearchResult(search_result),
        }
    }

    #[must_use]
    pub const fn artist_info(format: Format, artist_info: ArtistInfoResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::ArtistInfo(artist_info),
        }
    }

    #[must_use]
    pub const fn similar_songs(format: Format, similar_songs: SimilarSongsResponse) -> Self {
        Self {
            format,
            kind: ResponseKind::SimilarSongs(similar_songs),
        }
    }
}

impl IntoResponse for SubsonicResponse {
    fn into_response(self) -> Response {
        match self.format {
            Format::Xml => self.to_xml_response(),
            Format::Json => self.to_json_response(),
        }
    }
}

impl SubsonicResponse {
    #[allow(clippy::wrong_self_convention, clippy::too_many_lines)]
    fn to_xml_response(self) -> Response {
        let xml_result = match self.kind {
            ResponseKind::Empty => quick_xml::se::to_string(&xml::EmptyResponse::ok()),
            ResponseKind::License => quick_xml::se::to_string(&xml::LicenseResponse::ok()),
            ResponseKind::Error { code, message } => {
                quick_xml::se::to_string(&xml::ErrorResponse::new(code, message))
            }
            ResponseKind::OpenSubsonicExtensions(extensions) => {
                let xml_extensions = extensions
                    .into_iter()
                    .map(|ext| xml::OpenSubsonicExtensionXml {
                        name: ext.name,
                        versions: ext.versions,
                    })
                    .collect();
                quick_xml::se::to_string(&xml::OpenSubsonicExtensionsResponse::new(xml_extensions))
            }
            ResponseKind::MusicFolders(folders) => {
                quick_xml::se::to_string(&xml::MusicFoldersResponse::new(folders))
            }
            ResponseKind::Indexes(indexes) => {
                quick_xml::se::to_string(&xml::IndexesResponse::new(indexes))
            }
            ResponseKind::Artists(artists) => {
                quick_xml::se::to_string(&xml::ArtistsResponse::new(artists))
            }
            ResponseKind::Album(album) => quick_xml::se::to_string(&xml::AlbumResponse::new(album)),
            ResponseKind::Artist(artist) => {
                quick_xml::se::to_string(&xml::ArtistResponse::new(artist))
            }
            ResponseKind::Song(song) => quick_xml::se::to_string(&xml::SongResponse::new(song)),
            ResponseKind::AlbumList2(album_list2) => {
                quick_xml::se::to_string(&xml::AlbumList2Response::new(album_list2))
            }
            ResponseKind::Genres(genres) => {
                quick_xml::se::to_string(&xml::GenresResponse::new(genres))
            }
            ResponseKind::SearchResult3(search_result3) => {
                quick_xml::se::to_string(&xml::SearchResult3Response::new(search_result3))
            }
            ResponseKind::Starred2(starred2) => {
                quick_xml::se::to_string(&xml::Starred2Response::new(starred2))
            }
            ResponseKind::NowPlaying(now_playing) => {
                quick_xml::se::to_string(&xml::NowPlayingResponse::new(now_playing))
            }
            ResponseKind::RandomSongs(random_songs) => {
                quick_xml::se::to_string(&xml::RandomSongsResponse::new(random_songs))
            }
            ResponseKind::SongsByGenre(songs_by_genre) => {
                quick_xml::se::to_string(&xml::SongsByGenreResponse::new(songs_by_genre))
            }
            ResponseKind::Playlists(playlists) => {
                quick_xml::se::to_string(&xml::PlaylistsResponse::new(playlists))
            }
            ResponseKind::Playlist(playlist) => {
                quick_xml::se::to_string(&xml::PlaylistResponse::new(playlist))
            }
            ResponseKind::PlayQueue(play_queue) => {
                quick_xml::se::to_string(&xml::PlayQueueResponse::new(play_queue))
            }
            ResponseKind::PlayQueueByIndex(play_queue_by_index) => {
                quick_xml::se::to_string(&xml::PlayQueueByIndexResponse::new(play_queue_by_index))
            }
            ResponseKind::TokenInfo(token_info) => {
                quick_xml::se::to_string(&xml::TokenInfoResponse::new(token_info))
            }
            ResponseKind::User(user) => quick_xml::se::to_string(&xml::UserResponse::new(user)),
            ResponseKind::Users(users) => quick_xml::se::to_string(&xml::UsersResponse::new(users)),
            ResponseKind::ScanStatus(data) => {
                quick_xml::se::to_string(&xml::ScanStatusResponse::from_data(&data))
            }
            ResponseKind::Bookmarks => quick_xml::se::to_string(&xml::BookmarksResponse::new()),
            ResponseKind::ArtistInfo2(artist_info2) => {
                quick_xml::se::to_string(&xml::ArtistInfo2Response::new(artist_info2))
            }
            ResponseKind::AlbumInfo(album_info) => {
                quick_xml::se::to_string(&xml::AlbumInfoResponse::new(album_info))
            }
            ResponseKind::SimilarSongs2(similar_songs2) => {
                quick_xml::se::to_string(&xml::SimilarSongs2Response::new(similar_songs2))
            }
            ResponseKind::TopSongs(top_songs) => {
                quick_xml::se::to_string(&xml::TopSongsResponse::new(top_songs))
            }
            ResponseKind::Lyrics(lyrics) => {
                quick_xml::se::to_string(&xml::LyricsResponse::new(lyrics))
            }
            ResponseKind::LyricsList(lyrics_list) => {
                quick_xml::se::to_string(&xml::LyricsListResponse::new(lyrics_list))
            }
            ResponseKind::Directory(directory) => {
                quick_xml::se::to_string(&xml::DirectoryResponse::new(directory))
            }
            ResponseKind::AlbumList(album_list) => {
                quick_xml::se::to_string(&xml::AlbumListResponse::new(album_list))
            }
            ResponseKind::Starred(starred) => {
                quick_xml::se::to_string(&xml::StarredResponse::new(starred))
            }
            ResponseKind::SearchResult2(search_result2) => {
                quick_xml::se::to_string(&xml::SearchResult2Response::new(search_result2))
            }
            ResponseKind::SearchResult(search_result) => {
                quick_xml::se::to_string(&xml::SearchResultResponse::new(search_result))
            }
            ResponseKind::ArtistInfo(artist_info) => {
                quick_xml::se::to_string(&xml::ArtistInfoResponse::new(artist_info))
            }
            ResponseKind::SimilarSongs(similar_songs) => {
                quick_xml::se::to_string(&xml::SimilarSongsResponse::new(similar_songs))
            }
        };

        match xml_result {
            Ok(xml) => {
                let xml_with_declaration =
                    format!(r#"<?xml version="1.0" encoding="UTF-8"?>{xml}"#);
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
                    xml_with_declaration,
                )
                    .into_response()
            }
            Err(e) => {
                tracing::error!(error = %e, "XML serialization error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
        }
    }

    #[allow(clippy::wrong_self_convention, clippy::too_many_lines)]
    fn to_json_response(self) -> Response {
        let response = match self.kind {
            ResponseKind::Empty => json::SubsonicResponse::ok().wrap(),
            ResponseKind::License => json::SubsonicResponse::ok().with_license().wrap(),
            ResponseKind::Error { code, message } => {
                json::SubsonicResponse::error(code, message).wrap()
            }
            ResponseKind::OpenSubsonicExtensions(extensions) => json::SubsonicResponse::ok()
                .with_extensions(extensions)
                .wrap(),
            ResponseKind::MusicFolders(folders) => json::SubsonicResponse::ok()
                .with_music_folders(folders)
                .wrap(),
            ResponseKind::Indexes(indexes) => {
                json::SubsonicResponse::ok().with_indexes(indexes).wrap()
            }
            ResponseKind::Artists(artists) => {
                json::SubsonicResponse::ok().with_artists(artists).wrap()
            }
            ResponseKind::Album(album) => json::SubsonicResponse::ok().with_album(album).wrap(),
            ResponseKind::Artist(artist) => json::SubsonicResponse::ok().with_artist(artist).wrap(),
            ResponseKind::Song(song) => json::SubsonicResponse::ok().with_song(song).wrap(),
            ResponseKind::AlbumList2(album_list2) => json::SubsonicResponse::ok()
                .with_album_list2(album_list2)
                .wrap(),
            ResponseKind::Genres(genres) => json::SubsonicResponse::ok().with_genres(genres).wrap(),
            ResponseKind::SearchResult3(search_result3) => json::SubsonicResponse::ok()
                .with_search_result3(search_result3)
                .wrap(),
            ResponseKind::Starred2(starred2) => {
                json::SubsonicResponse::ok().with_starred2(starred2).wrap()
            }
            ResponseKind::NowPlaying(now_playing) => json::SubsonicResponse::ok()
                .with_now_playing(now_playing)
                .wrap(),
            ResponseKind::RandomSongs(random_songs) => json::SubsonicResponse::ok()
                .with_random_songs(random_songs)
                .wrap(),
            ResponseKind::SongsByGenre(songs_by_genre) => json::SubsonicResponse::ok()
                .with_songs_by_genre(songs_by_genre)
                .wrap(),
            ResponseKind::Playlists(playlists) => json::SubsonicResponse::ok()
                .with_playlists(playlists)
                .wrap(),
            ResponseKind::Playlist(playlist) => {
                json::SubsonicResponse::ok().with_playlist(playlist).wrap()
            }
            ResponseKind::PlayQueue(play_queue) => json::SubsonicResponse::ok()
                .with_play_queue(play_queue)
                .wrap(),
            ResponseKind::PlayQueueByIndex(play_queue_by_index) => json::SubsonicResponse::ok()
                .with_play_queue_by_index(play_queue_by_index)
                .wrap(),
            ResponseKind::TokenInfo(token_info) => json::SubsonicResponse::ok()
                .with_token_info(token_info)
                .wrap(),
            ResponseKind::User(user) => json::SubsonicResponse::ok().with_user(user).wrap(),
            ResponseKind::Users(users) => json::SubsonicResponse::ok().with_users(users).wrap(),
            ResponseKind::ScanStatus(data) => {
                json::SubsonicResponse::ok().with_scan_status(&data).wrap()
            }
            ResponseKind::Bookmarks => json::SubsonicResponse::ok().with_bookmarks().wrap(),
            ResponseKind::ArtistInfo2(artist_info2) => json::SubsonicResponse::ok()
                .with_artist_info2(artist_info2)
                .wrap(),
            ResponseKind::AlbumInfo(album_info) => json::SubsonicResponse::ok()
                .with_album_info(album_info)
                .wrap(),
            ResponseKind::SimilarSongs2(similar_songs2) => json::SubsonicResponse::ok()
                .with_similar_songs2(similar_songs2)
                .wrap(),
            ResponseKind::TopSongs(top_songs) => json::SubsonicResponse::ok()
                .with_top_songs(top_songs)
                .wrap(),
            ResponseKind::Lyrics(lyrics) => json::SubsonicResponse::ok().with_lyrics(lyrics).wrap(),
            ResponseKind::LyricsList(lyrics_list) => json::SubsonicResponse::ok()
                .with_lyrics_list(lyrics_list)
                .wrap(),
            ResponseKind::Directory(directory) => json::SubsonicResponse::ok()
                .with_directory(directory)
                .wrap(),
            ResponseKind::AlbumList(album_list) => json::SubsonicResponse::ok()
                .with_album_list(album_list)
                .wrap(),
            ResponseKind::Starred(starred) => {
                json::SubsonicResponse::ok().with_starred(starred).wrap()
            }
            ResponseKind::SearchResult2(search_result2) => json::SubsonicResponse::ok()
                .with_search_result2(search_result2)
                .wrap(),
            ResponseKind::SearchResult(search_result) => json::SubsonicResponse::ok()
                .with_search_result(search_result)
                .wrap(),
            ResponseKind::ArtistInfo(artist_info) => json::SubsonicResponse::ok()
                .with_artist_info(artist_info)
                .wrap(),
            ResponseKind::SimilarSongs(similar_songs) => json::SubsonicResponse::ok()
                .with_similar_songs(similar_songs)
                .wrap(),
        };

        match serde_json::to_string(&response) {
            Ok(json) => {
                // Transform JSON keys: remove @ prefix and convert $text to $value
                let transformed = transform_json_keys(&json);
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                    transformed,
                )
                    .into_response()
            }
            Err(e) => {
                tracing::error!(error = %e, "JSON serialization error");
                (StatusCode::INTERNAL_SERVER_ERROR, "Internal server error").into_response()
            }
        }
    }
}

/// Transform JSON keys to match Subsonic API expectations:
/// - Remove @ prefix from attribute keys
/// - Convert $text to value (for genre text content)
fn transform_json_keys(json: &str) -> String {
    // Parse as Value, transform, and re-serialize
    serde_json::from_str::<serde_json::Value>(json).map_or_else(
        |_| json.to_string(),
        |value| {
            let transformed = transform_value(value);
            serde_json::to_string(&transformed).unwrap_or_else(|_| json.to_string())
        },
    )
}

fn transform_value(value: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;

    match value {
        Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (key, val) in map {
                let new_key = key.strip_prefix('@').map_or_else(
                    || {
                        if key == "$text" {
                            "value".to_string()
                        } else {
                            key.clone()
                        }
                    },
                    std::string::ToString::to_string,
                );
                new_map.insert(new_key, transform_value(val));
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(transform_value).collect()),
        other => other,
    }
}

// ============================================================================
// Helper functions
// ============================================================================

/// Helper function to create an empty successful response.
#[must_use]
pub const fn ok_empty(format: Format) -> SubsonicResponse {
    SubsonicResponse::empty(format)
}

/// Helper function to create a license response.
#[must_use]
pub const fn ok_license(format: Format) -> SubsonicResponse {
    SubsonicResponse::license(format)
}

/// Helper function to create an error response.
#[must_use]
pub fn error_response(format: Format, error: &ApiError) -> SubsonicResponse {
    SubsonicResponse::error(format, error)
}

/// Helper function to create an `OpenSubsonic` extensions response.
#[must_use]
pub fn ok_open_subsonic_extensions(format: Format) -> SubsonicResponse {
    SubsonicResponse::open_subsonic_extensions(format, supported_extensions())
}

/// Helper function to create a music folders response.
#[must_use]
pub const fn ok_music_folders(
    format: Format,
    folders: Vec<MusicFolderResponse>,
) -> SubsonicResponse {
    SubsonicResponse::music_folders(format, folders)
}

/// Helper function to create an indexes response.
#[must_use]
pub const fn ok_indexes(format: Format, indexes: IndexesResponse) -> SubsonicResponse {
    SubsonicResponse::indexes(format, indexes)
}

/// Helper function to create an artists response.
#[must_use]
pub const fn ok_artists(format: Format, artists: ArtistsID3Response) -> SubsonicResponse {
    SubsonicResponse::artists(format, artists)
}

/// Helper function to create an album response.
#[must_use]
pub const fn ok_album(format: Format, album: AlbumWithSongsID3Response) -> SubsonicResponse {
    SubsonicResponse::album(format, album)
}

/// Helper function to create an artist response.
#[must_use]
pub const fn ok_artist(format: Format, artist: ArtistWithAlbumsID3Response) -> SubsonicResponse {
    SubsonicResponse::artist(format, artist)
}

/// Helper function to create a song response.
#[must_use]
pub const fn ok_song(format: Format, song: ChildResponse) -> SubsonicResponse {
    SubsonicResponse::song(format, song)
}

/// Helper function to create an album list2 response.
#[must_use]
pub const fn ok_album_list2(format: Format, album_list2: AlbumList2Response) -> SubsonicResponse {
    SubsonicResponse::album_list2(format, album_list2)
}

/// Helper function to create a genres response.
#[must_use]
pub const fn ok_genres(format: Format, genres: GenresResponse) -> SubsonicResponse {
    SubsonicResponse::genres(format, genres)
}

/// Helper function to create a search result3 response.
#[must_use]
pub const fn ok_search_result3(
    format: Format,
    search_result3: SearchResult3Response,
) -> SubsonicResponse {
    SubsonicResponse::search_result3(format, search_result3)
}

/// Helper function to create a starred2 response.
#[must_use]
pub const fn ok_starred2(format: Format, starred2: Starred2Response) -> SubsonicResponse {
    SubsonicResponse::starred2(format, starred2)
}

/// Helper function to create a now playing response.
#[must_use]
pub const fn ok_now_playing(format: Format, now_playing: NowPlayingResponse) -> SubsonicResponse {
    SubsonicResponse::now_playing(format, now_playing)
}

/// Helper function to create a random songs response.
#[must_use]
pub const fn ok_random_songs(
    format: Format,
    random_songs: RandomSongsResponse,
) -> SubsonicResponse {
    SubsonicResponse::random_songs(format, random_songs)
}

/// Helper function to create a songs by genre response.
#[must_use]
pub const fn ok_songs_by_genre(
    format: Format,
    songs_by_genre: SongsByGenreResponse,
) -> SubsonicResponse {
    SubsonicResponse::songs_by_genre(format, songs_by_genre)
}

/// Helper function to create a playlists response.
#[must_use]
pub const fn ok_playlists(format: Format, playlists: PlaylistsResponse) -> SubsonicResponse {
    SubsonicResponse::playlists(format, playlists)
}

/// Helper function to create a playlist with songs response.
#[must_use]
pub const fn ok_playlist(format: Format, playlist: PlaylistWithSongsResponse) -> SubsonicResponse {
    SubsonicResponse::playlist(format, playlist)
}

/// Helper function to create a play queue response.
#[must_use]
pub const fn ok_play_queue(format: Format, play_queue: PlayQueueResponse) -> SubsonicResponse {
    SubsonicResponse::play_queue(format, play_queue)
}

/// Helper function to create a play queue by index response (`OpenSubsonic`).
#[must_use]
pub const fn ok_play_queue_by_index(
    format: Format,
    play_queue_by_index: PlayQueueByIndexResponse,
) -> SubsonicResponse {
    SubsonicResponse::play_queue_by_index(format, play_queue_by_index)
}

/// Helper function to create a token info response (`OpenSubsonic`).
#[must_use]
pub const fn ok_token_info(format: Format, token_info: TokenInfoResponse) -> SubsonicResponse {
    SubsonicResponse::token_info(format, token_info)
}

/// Helper function to create a user response.
#[must_use]
pub const fn ok_user(format: Format, user: UserResponse) -> SubsonicResponse {
    SubsonicResponse::user(format, user)
}

/// Helper function to create a users response.
#[must_use]
pub const fn ok_users(format: Format, users: UsersResponse) -> SubsonicResponse {
    SubsonicResponse::users(format, users)
}

/// Helper function to create a scan status response.
#[must_use]
pub const fn ok_scan_status(format: Format, data: ScanStatusData) -> SubsonicResponse {
    SubsonicResponse::scan_status(format, data)
}

/// Helper function to create an empty bookmarks response.
#[must_use]
pub const fn ok_bookmarks(format: Format) -> SubsonicResponse {
    SubsonicResponse::bookmarks(format)
}

/// Helper function to create an artist info2 response.
#[must_use]
pub const fn ok_artist_info2(
    format: Format,
    artist_info2: ArtistInfo2Response,
) -> SubsonicResponse {
    SubsonicResponse::artist_info2(format, artist_info2)
}

/// Helper function to create an album info response.
#[must_use]
pub const fn ok_album_info(format: Format, album_info: AlbumInfoResponse) -> SubsonicResponse {
    SubsonicResponse::album_info(format, album_info)
}

/// Helper function to create a similar songs2 response.
#[must_use]
pub const fn ok_similar_songs2(
    format: Format,
    similar_songs2: SimilarSongs2Response,
) -> SubsonicResponse {
    SubsonicResponse::similar_songs2(format, similar_songs2)
}

/// Helper function to create a top songs response.
#[must_use]
pub const fn ok_top_songs(format: Format, top_songs: TopSongsResponse) -> SubsonicResponse {
    SubsonicResponse::top_songs(format, top_songs)
}

/// Helper function to create a lyrics response.
#[must_use]
pub const fn ok_lyrics(format: Format, lyrics: LyricsResponse) -> SubsonicResponse {
    SubsonicResponse::lyrics(format, lyrics)
}

/// Helper function to create a lyrics list response (getLyricsBySongId - `OpenSubsonic`).
#[must_use]
pub const fn ok_lyrics_list(format: Format, lyrics_list: LyricsListResponse) -> SubsonicResponse {
    SubsonicResponse::lyrics_list(format, lyrics_list)
}

/// Helper function to create a directory response (getMusicDirectory).
#[must_use]
pub const fn ok_directory(format: Format, directory: DirectoryResponse) -> SubsonicResponse {
    SubsonicResponse::directory(format, directory)
}

/// Helper function to create an album list response (getAlbumList).
#[must_use]
pub const fn ok_album_list(format: Format, album_list: AlbumListResponse) -> SubsonicResponse {
    SubsonicResponse::album_list(format, album_list)
}

/// Helper function to create a starred response (getStarred).
#[must_use]
pub const fn ok_starred(format: Format, starred: StarredResponse) -> SubsonicResponse {
    SubsonicResponse::starred(format, starred)
}

/// Helper function to create a search result2 response (search2).
#[must_use]
pub const fn ok_search_result2(
    format: Format,
    search_result2: SearchResult2Response,
) -> SubsonicResponse {
    SubsonicResponse::search_result2(format, search_result2)
}

/// Helper function to create a search result response (search).
#[must_use]
pub const fn ok_search_result(
    format: Format,
    search_result: SearchResultResponse,
) -> SubsonicResponse {
    SubsonicResponse::search_result(format, search_result)
}

/// Helper function to create an artist info response (getArtistInfo).
#[must_use]
pub const fn ok_artist_info(format: Format, artist_info: ArtistInfoResponse) -> SubsonicResponse {
    SubsonicResponse::artist_info(format, artist_info)
}

/// Helper function to create a similar songs response (getSimilarSongs).
#[must_use]
pub const fn ok_similar_songs(
    format: Format,
    similar_songs: SimilarSongsResponse,
) -> SubsonicResponse {
    SubsonicResponse::similar_songs(format, similar_songs)
}
