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
    RemoteCommandsResponse, RemoteSessionResponse, RemoteStateResponse, SearchResult2Response,
    SearchResult3Response, SearchResultResponse, SimilarSongs2Response, SimilarSongsResponse,
    SongsByGenreResponse, Starred2Response, StarredResponse, TokenInfoResponse, TopSongsResponse,
};
use crate::models::user::{UserResponse, UsersResponse};
use crate::scanner::ScanPhase;

/// The current Subsonic API version we're compatible with.
pub const API_VERSION: &str = "1.16.1";

/// Server name reported in responses.
pub const SERVER_NAME: &str = "suboxide";

/// Server version from Cargo.toml.
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The format of the response returned to the client.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Format {
    /// XML format (Subsonic default).
    #[default]
    Xml,
    /// JSON format.
    Json,
}

impl Format {
    /// Get the format from the `f` query parameter.
    pub fn from_param(f: Option<&str>) -> Result<Self, String> {
        match f {
            Some(format) if format.eq_ignore_ascii_case("json") => Ok(Self::Json),
            Some(format) if format.eq_ignore_ascii_case("xml") => Ok(Self::Xml),
            None => Ok(Self::Xml),
            Some(format) => Err(format.to_string()),
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

/// Status of a media library scan.
#[derive(Debug, Clone, Default)]
pub struct ScanStatusData {
    /// Whether a scan is currently in progress.
    pub scanning: bool,
    /// Number of items scanned so far.
    pub count: u64,
    /// Total number of items to scan.
    pub total: u64,
    /// Current scan phase.
    pub phase: ScanPhase,
    /// Current folder being scanned.
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
    pub fn new(name: impl Into<String>, versions: &[i32]) -> Self {
        Self {
            name: name.into(),
            versions: versions.to_vec(),
        }
    }
}

/// Returns the list of `OpenSubsonic` extensions supported by this server.
#[must_use]
pub fn supported_extensions() -> Vec<OpenSubsonicExtension> {
    vec![
        OpenSubsonicExtension::new("apiKeyAuthentication", &[1]),
        OpenSubsonicExtension::new("songLyrics", &[1]),
        OpenSubsonicExtension::new("remoteControl", &[1]),
    ]
}

// ============================================================================
// XML Response Types (use @attribute naming)
// ============================================================================

mod xml {
    use super::{API_VERSION, ResponseStatus, SERVER_NAME, SERVER_VERSION, Serialize};

    // Note: quick_xml doesn't support #[serde(flatten)], so we need to include
    // all base attributes directly in each response struct.

    // Generates an XML response struct with the standard Subsonic base attributes.
    macro_rules! xml_response {
        // Empty response (no content fields).
        (pub struct $name:ident) => {
            #[derive(Debug, Serialize)]
            #[serde(rename = "subsonic-response")]
            pub struct $name {
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

            impl $name {
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
        };

        // Single-field response with a `new` constructor.
        (
            pub struct $name:ident {
                $(#[$field_meta:meta])*
                $field_vis:vis $field_name:ident: $field_ty:ty $(,)?
            }
        ) => {
            #[derive(Debug, Serialize)]
            #[serde(rename = "subsonic-response")]
            pub struct $name {
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
                $(#[$field_meta])*
                $field_vis $field_name: $field_ty,
            }

            impl $name {
                pub const fn new($field_name: $field_ty) -> Self {
                    Self {
                        xmlns: "http://subsonic.org/restapi",
                        status: ResponseStatus::Ok,
                        version: API_VERSION,
                        server_type: SERVER_NAME,
                        server_version: SERVER_VERSION,
                        open_subsonic: true,
                        $field_name,
                    }
                }
            }
        };
    }

    xml_response!(pub struct EmptyResponse);

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
    pub struct OpenSubsonicExtensionsXml {
        #[serde(rename = "openSubsonicExtension")]
        pub extensions: Vec<OpenSubsonicExtensionXml>,
    }

    xml_response! {
        pub struct OpenSubsonicExtensionsResponse {
            #[serde(rename = "openSubsonicExtensions")]
            pub extensions: OpenSubsonicExtensionsXml
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

    xml_response! {
        pub struct IndexesResponse {
            #[serde(rename = "indexes")]
            pub indexes: super::IndexesResponse
        }
    }

    xml_response! {
        pub struct ArtistsResponse {
            #[serde(rename = "artists")]
            pub artists: super::ArtistsID3Response
        }
    }

    xml_response! {
        pub struct AlbumResponse {
            #[serde(rename = "album")]
            pub album: super::AlbumWithSongsID3Response
        }
    }

    xml_response! {
        pub struct ArtistResponse {
            #[serde(rename = "artist")]
            pub artist: super::ArtistWithAlbumsID3Response
        }
    }

    xml_response! {
        pub struct SongResponse {
            #[serde(rename = "song")]
            pub song: super::ChildResponse
        }
    }

    xml_response! {
        pub struct AlbumList2Response {
            #[serde(rename = "albumList2")]
            pub album_list2: super::AlbumList2Response
        }
    }

    xml_response! {
        pub struct GenresResponse {
            #[serde(rename = "genres")]
            pub genres: super::GenresResponse
        }
    }

    xml_response! {
        pub struct SearchResult3Response {
            #[serde(rename = "searchResult3")]
            pub search_result3: super::SearchResult3Response
        }
    }

    xml_response! {
        pub struct Starred2Response {
            #[serde(rename = "starred2")]
            pub starred2: super::Starred2Response
        }
    }

    xml_response! {
        pub struct NowPlayingResponse {
            #[serde(rename = "nowPlaying")]
            pub now_playing: super::NowPlayingResponse
        }
    }

    xml_response! {
        pub struct RandomSongsResponse {
            #[serde(rename = "randomSongs")]
            pub random_songs: super::RandomSongsResponse
        }
    }

    xml_response! {
        pub struct SongsByGenreResponse {
            #[serde(rename = "songsByGenre")]
            pub songs_by_genre: super::SongsByGenreResponse
        }
    }

    xml_response! {
        pub struct PlaylistsResponse {
            #[serde(rename = "playlists")]
            pub playlists: super::PlaylistsResponse
        }
    }

    xml_response! {
        pub struct PlaylistResponse {
            #[serde(rename = "playlist")]
            pub playlist: super::PlaylistWithSongsResponse
        }
    }

    xml_response! {
        pub struct PlayQueueResponse {
            #[serde(rename = "playQueue")]
            pub play_queue: super::PlayQueueResponse
        }
    }

    xml_response! {
        pub struct PlayQueueByIndexResponse {
            #[serde(rename = "playQueueByIndex")]
            pub play_queue_by_index: super::PlayQueueByIndexResponse
        }
    }

    xml_response! {
        pub struct TokenInfoResponse {
            #[serde(rename = "tokenInfo")]
            pub token_info: super::TokenInfoResponse
        }
    }

    xml_response! {
        pub struct UserResponse {
            #[serde(rename = "user")]
            pub user: super::UserResponse
        }
    }

    xml_response! {
        pub struct UsersResponse {
            #[serde(rename = "users")]
            pub users: super::UsersResponse
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
                    Some(data.phase.as_str().to_string())
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

    xml_response! {
        pub struct ArtistInfo2Response {
            #[serde(rename = "artistInfo2")]
            pub artist_info2: super::ArtistInfo2Response
        }
    }

    xml_response! {
        pub struct AlbumInfoResponse {
            #[serde(rename = "albumInfo")]
            pub album_info: super::AlbumInfoResponse
        }
    }

    xml_response! {
        pub struct SimilarSongs2Response {
            #[serde(rename = "similarSongs2")]
            pub similar_songs2: super::SimilarSongs2Response
        }
    }

    xml_response! {
        pub struct TopSongsResponse {
            #[serde(rename = "topSongs")]
            pub top_songs: super::TopSongsResponse
        }
    }

    xml_response! {
        pub struct LyricsResponse {
            #[serde(rename = "lyrics")]
            pub lyrics: super::LyricsResponse
        }
    }

    xml_response! {
        pub struct LyricsListResponse {
            #[serde(rename = "lyricsList")]
            pub lyrics_list: super::LyricsListResponse
        }
    }

    xml_response! {
        pub struct DirectoryResponse {
            #[serde(rename = "directory")]
            pub directory: super::DirectoryResponse
        }
    }

    xml_response! {
        pub struct AlbumListResponse {
            #[serde(rename = "albumList")]
            pub album_list: super::AlbumListResponse
        }
    }

    xml_response! {
        pub struct StarredResponse {
            #[serde(rename = "starred")]
            pub starred: super::StarredResponse
        }
    }

    xml_response! {
        pub struct SearchResult2Response {
            #[serde(rename = "searchResult2")]
            pub search_result2: super::SearchResult2Response
        }
    }

    xml_response! {
        pub struct SearchResultResponse {
            #[serde(rename = "searchResult")]
            pub search_result: super::SearchResultResponse
        }
    }

    xml_response! {
        pub struct ArtistInfoResponse {
            #[serde(rename = "artistInfo")]
            pub artist_info: super::ArtistInfoResponse
        }
    }

    xml_response! {
        pub struct SimilarSongsResponse {
            #[serde(rename = "similarSongs")]
            pub similar_songs: super::SimilarSongsResponse
        }
    }

    xml_response! {
        pub struct RemoteSessionResponse {
            #[serde(rename = "remoteSession")]
            pub remote_session: super::RemoteSessionResponse
        }
    }

    xml_response! {
        pub struct RemoteCommandsResponse {
            #[serde(rename = "remoteCommands")]
            pub remote_commands: super::RemoteCommandsResponse
        }
    }

    xml_response! {
        pub struct RemoteStateResponse {
            #[serde(rename = "remoteState")]
            pub remote_state: super::RemoteStateResponse
        }
    }
}

mod json {
    use super::Serialize;

    #[derive(Debug, Serialize)]
    pub struct Empty;

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
                    Some(data.phase.as_str().to_string())
                } else {
                    None
                },
                folder: data.folder.clone(),
            }
        }
    }

    #[derive(Debug, Serialize)]
    pub struct MusicFolders {
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
}

// ============================================================================
// Format-aware Response Types
// ============================================================================

/// A Subsonic API response that can be serialized to XML or JSON.
#[derive(Debug)]
pub struct SubsonicResponse {
    format: Format,
    kind: ResponseKind,
}

#[expect(
    clippy::large_enum_variant,
    reason = "Enum keeps format-specific payload variants in one allocation-free response type"
)]
#[derive(Debug)]
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
    RemoteSession(RemoteSessionResponse),
    RemoteCommands(RemoteCommandsResponse),
    RemoteState(RemoteStateResponse),
}

macro_rules! response_constructor {
    ($name:ident, $payload:ty, $variant:ident) => {
        #[must_use]
        pub const fn $name(format: Format, payload: $payload) -> Self {
            Self::new(format, ResponseKind::$variant(payload))
        }
    };
}

impl SubsonicResponse {
    const fn new(format: Format, kind: ResponseKind) -> Self {
        Self { format, kind }
    }

    #[must_use]
    pub const fn empty(format: Format) -> Self {
        Self::new(format, ResponseKind::Empty)
    }

    #[must_use]
    pub const fn license(format: Format) -> Self {
        Self::new(format, ResponseKind::License)
    }

    #[must_use]
    pub fn error(format: Format, error: &ApiError) -> Self {
        Self::new(
            format,
            ResponseKind::Error {
                code: error.code(),
                message: error.message(),
            },
        )
    }

    #[must_use]
    pub const fn bookmarks(format: Format) -> Self {
        Self::new(format, ResponseKind::Bookmarks)
    }

    response_constructor!(
        open_subsonic_extensions,
        Vec<OpenSubsonicExtension>,
        OpenSubsonicExtensions
    );
    response_constructor!(music_folders, Vec<MusicFolderResponse>, MusicFolders);
    response_constructor!(indexes, IndexesResponse, Indexes);
    response_constructor!(artists, ArtistsID3Response, Artists);
    response_constructor!(album, AlbumWithSongsID3Response, Album);
    response_constructor!(artist, ArtistWithAlbumsID3Response, Artist);
    response_constructor!(song, ChildResponse, Song);
    response_constructor!(album_list2, AlbumList2Response, AlbumList2);
    response_constructor!(genres, GenresResponse, Genres);
    response_constructor!(search_result3, SearchResult3Response, SearchResult3);
    response_constructor!(starred2, Starred2Response, Starred2);
    response_constructor!(now_playing, NowPlayingResponse, NowPlaying);
    response_constructor!(random_songs, RandomSongsResponse, RandomSongs);
    response_constructor!(songs_by_genre, SongsByGenreResponse, SongsByGenre);
    response_constructor!(playlists, PlaylistsResponse, Playlists);
    response_constructor!(playlist, PlaylistWithSongsResponse, Playlist);
    response_constructor!(play_queue, PlayQueueResponse, PlayQueue);
    response_constructor!(
        play_queue_by_index,
        PlayQueueByIndexResponse,
        PlayQueueByIndex
    );
    response_constructor!(token_info, TokenInfoResponse, TokenInfo);
    response_constructor!(user, UserResponse, User);
    response_constructor!(users, UsersResponse, Users);
    response_constructor!(scan_status, ScanStatusData, ScanStatus);
    response_constructor!(artist_info2, ArtistInfo2Response, ArtistInfo2);
    response_constructor!(album_info, AlbumInfoResponse, AlbumInfo);
    response_constructor!(similar_songs2, SimilarSongs2Response, SimilarSongs2);
    response_constructor!(top_songs, TopSongsResponse, TopSongs);
    response_constructor!(lyrics, LyricsResponse, Lyrics);
    response_constructor!(lyrics_list, LyricsListResponse, LyricsList);
    response_constructor!(directory, DirectoryResponse, Directory);
    response_constructor!(album_list, AlbumListResponse, AlbumList);
    response_constructor!(starred, StarredResponse, Starred);
    response_constructor!(search_result2, SearchResult2Response, SearchResult2);
    response_constructor!(search_result, SearchResultResponse, SearchResult);
    response_constructor!(artist_info, ArtistInfoResponse, ArtistInfo);
    response_constructor!(similar_songs, SimilarSongsResponse, SimilarSongs);
    response_constructor!(remote_session, RemoteSessionResponse, RemoteSession);
    response_constructor!(remote_commands, RemoteCommandsResponse, RemoteCommands);
    response_constructor!(remote_state, RemoteStateResponse, RemoteState);
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
    #[expect(
        clippy::wrong_self_convention,
        reason = "This method consumes self to avoid cloning large response payloads"
    )]
    #[expect(
        clippy::too_many_lines,
        reason = "Serialization dispatch covers all Subsonic response variants"
    )]
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
                quick_xml::se::to_string(&xml::OpenSubsonicExtensionsResponse::new(
                    xml::OpenSubsonicExtensionsXml {
                        extensions: xml_extensions,
                    },
                ))
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
            ResponseKind::RemoteSession(remote_session) => {
                quick_xml::se::to_string(&xml::RemoteSessionResponse::new(remote_session))
            }
            ResponseKind::RemoteCommands(remote_commands) => {
                quick_xml::se::to_string(&xml::RemoteCommandsResponse::new(remote_commands))
            }
            ResponseKind::RemoteState(remote_state) => {
                quick_xml::se::to_string(&xml::RemoteStateResponse::new(remote_state))
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
                tracing::event!(
                    name: "api.response.serialize_xml.failed",
                    tracing::Level::ERROR,
                    error = %e,
                    "xml serialization failed"
                );
                let body = r#"<?xml version="1.0" encoding="UTF-8"?><subsonic-response xmlns="http://subsonic.org/restapi" status="failed" version="1.16.1" type="suboxide"><error code="0" message="Internal server error"/></subsonic-response>"#;
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    [(header::CONTENT_TYPE, "application/xml; charset=utf-8")],
                    body,
                )
                    .into_response()
            }
        }
    }

    #[expect(
        clippy::wrong_self_convention,
        reason = "This method consumes self to avoid cloning large response payloads"
    )]
    fn to_json_response(self) -> Response {
        let response = match self.kind {
            ResponseKind::Empty => Ok(json_empty()),
            ResponseKind::License => json_named("license", json::License { valid: true }),
            ResponseKind::Error { code, message } => json_error(code, message),
            ResponseKind::OpenSubsonicExtensions(extensions) => {
                json_named("openSubsonicExtensions", extensions)
            }
            ResponseKind::MusicFolders(folders) => {
                json_named("musicFolders", json::MusicFolders { folders })
            }
            ResponseKind::Indexes(indexes) => json_named("indexes", indexes),
            ResponseKind::Artists(artists) => json_named("artists", artists),
            ResponseKind::Album(album) => json_named("album", album),
            ResponseKind::Artist(artist) => json_named("artist", artist),
            ResponseKind::Song(song) => json_named("song", song),
            ResponseKind::AlbumList2(album_list2) => json_named("albumList2", album_list2),
            ResponseKind::Genres(genres) => json_named("genres", genres),
            ResponseKind::SearchResult3(search_result3) => {
                json_named("searchResult3", search_result3)
            }
            ResponseKind::Starred2(starred2) => json_named("starred2", starred2),
            ResponseKind::NowPlaying(now_playing) => json_named("nowPlaying", now_playing),
            ResponseKind::RandomSongs(random_songs) => json_named("randomSongs", random_songs),
            ResponseKind::SongsByGenre(songs_by_genre) => {
                json_named("songsByGenre", songs_by_genre)
            }
            ResponseKind::Playlists(playlists) => json_named("playlists", playlists),
            ResponseKind::Playlist(playlist) => json_named("playlist", playlist),
            ResponseKind::PlayQueue(play_queue) => json_named("playQueue", play_queue),
            ResponseKind::PlayQueueByIndex(play_queue_by_index) => {
                json_named("playQueueByIndex", play_queue_by_index)
            }
            ResponseKind::TokenInfo(token_info) => json_named("tokenInfo", token_info),
            ResponseKind::User(user) => json_named("user", user),
            ResponseKind::Users(users) => json_named("users", users),
            ResponseKind::ScanStatus(data) => {
                json_named("scanStatus", json::ScanStatusJson::from_data(&data))
            }
            ResponseKind::Bookmarks => json_named("bookmarks", json::Empty),
            ResponseKind::ArtistInfo2(artist_info2) => json_named("artistInfo2", artist_info2),
            ResponseKind::AlbumInfo(album_info) => json_named("albumInfo", album_info),
            ResponseKind::SimilarSongs2(similar_songs2) => {
                json_named("similarSongs2", similar_songs2)
            }
            ResponseKind::TopSongs(top_songs) => json_named("topSongs", top_songs),
            ResponseKind::Lyrics(lyrics) => json_named("lyrics", lyrics),
            ResponseKind::LyricsList(lyrics_list) => json_named("lyricsList", lyrics_list),
            ResponseKind::Directory(directory) => json_named("directory", directory),
            ResponseKind::AlbumList(album_list) => json_named("albumList", album_list),
            ResponseKind::Starred(starred) => json_named("starred", starred),
            ResponseKind::SearchResult2(search_result2) => {
                json_named("searchResult2", search_result2)
            }
            ResponseKind::SearchResult(search_result) => json_named("searchResult", search_result),
            ResponseKind::ArtistInfo(artist_info) => json_named("artistInfo", artist_info),
            ResponseKind::SimilarSongs(similar_songs) => json_named("similarSongs", similar_songs),
            ResponseKind::RemoteSession(remote_session) => {
                json_named("remoteSession", remote_session)
            }
            ResponseKind::RemoteCommands(remote_commands) => {
                json_named("remoteCommands", remote_commands)
            }
            ResponseKind::RemoteState(remote_state) => json_named("remoteState", remote_state),
        };

        match response.and_then(|response| serde_json::to_string(&response)) {
            Ok(json) => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
                json,
            )
                .into_response(),
            Err(e) => {
                tracing::event!(
                    name: "api.response.serialize_json.failed",
                    tracing::Level::ERROR,
                    error = %e,
                    "json serialization failed"
                );
                json_internal_error_response()
            }
        }
    }
}

fn json_base(status: ResponseStatus) -> serde_json::Map<String, serde_json::Value> {
    let mut response = serde_json::Map::new();
    response.insert("status".into(), serde_json::json!(status));
    response.insert("version".into(), serde_json::json!(API_VERSION));
    response.insert("type".into(), serde_json::json!(SERVER_NAME));
    response.insert("serverVersion".into(), serde_json::json!(SERVER_VERSION));
    response.insert("openSubsonic".into(), serde_json::json!(true));
    response
}

fn json_wrap(response: serde_json::Map<String, serde_json::Value>) -> serde_json::Value {
    let mut wrapper = serde_json::Map::new();
    wrapper.insert(
        "subsonic-response".into(),
        serde_json::Value::Object(response),
    );
    serde_json::Value::Object(wrapper)
}

fn json_empty() -> serde_json::Value {
    json_wrap(json_base(ResponseStatus::Ok))
}

fn json_named<T: Serialize>(
    name: &'static str,
    payload: T,
) -> Result<serde_json::Value, serde_json::Error> {
    let mut response = json_base(ResponseStatus::Ok);
    response.insert(name.into(), transform_value(serde_json::to_value(payload)?));
    Ok(json_wrap(response))
}

fn json_error(code: u32, message: String) -> Result<serde_json::Value, serde_json::Error> {
    let mut response = json_base(ResponseStatus::Failed);
    response.insert(
        "error".into(),
        serde_json::to_value(json::ErrorDetail { code, message })?,
    );
    Ok(json_wrap(response))
}

fn json_internal_error_response() -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
        r#"{"subsonic-response":{"status":"failed","version":"1.16.1","type":"suboxide","error":{"code":0,"message":"Internal server error"}}}"#,
    )
        .into_response()
}

/// Transform JSON keys to match Subsonic API expectations:
/// - Remove @ prefix from attribute keys
/// - Convert $text to value (for genre text content)
#[cfg(test)]
fn transform_json_keys(json: &str) -> Result<String, serde_json::Error> {
    // Parse as Value, transform, and re-serialize
    let value = serde_json::from_str::<serde_json::Value>(json)?;
    let transformed = transform_value(value);
    serde_json::to_string(&transformed)
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

/// Create an error response.
#[must_use]
pub fn error_response(format: Format, error: &ApiError) -> SubsonicResponse {
    SubsonicResponse::error(format, error)
}

#[cfg(test)]
mod tests {
    use super::{Format, supported_extensions};

    #[test]
    fn format_from_param_accepts_only_json_like_formats() {
        assert_eq!(Format::from_param(Some("json")), Ok(Format::Json));
        assert_eq!(Format::from_param(Some("JSON")), Ok(Format::Json));
        assert_eq!(Format::from_param(Some("xml")), Ok(Format::Xml));
        assert_eq!(Format::from_param(Some("XML")), Ok(Format::Xml));
        assert_eq!(Format::from_param(None), Ok(Format::Xml));
        assert_eq!(Format::from_param(Some("jsonp")), Err("jsonp".to_string()));
    }

    #[test]
    fn supported_extensions_are_stable_and_sorted_by_contract() {
        let extensions = supported_extensions();
        let names: Vec<_> = extensions
            .iter()
            .map(|extension| extension.name.as_str())
            .collect();

        assert_eq!(
            names,
            ["apiKeyAuthentication", "songLyrics", "remoteControl"]
        );
        assert!(extensions.iter().all(|extension| extension.versions == [1]));
    }

    #[test]
    fn supported_extensions_do_not_advertise_form_post() {
        let extensions = supported_extensions();

        assert!(
            !extensions
                .iter()
                .any(|extension| extension.name == "formPost"),
            "formPost requires reading endpoint parameters from request bodies"
        );
    }

    #[test]
    fn transform_json_keys_recurses_through_objects_and_arrays() {
        let json = r#"{"@id":"root","child":[{"@name":"genre","$text":"Jazz"}]}"#;

        let transformed = super::transform_json_keys(json).expect("valid JSON should transform");

        assert_eq!(
            transformed,
            r#"{"child":[{"name":"genre","value":"Jazz"}],"id":"root"}"#
        );
    }

    #[test]
    fn transform_json_keys_rejects_invalid_json() {
        let invalid = "not json";

        assert!(super::transform_json_keys(invalid).is_err());
    }
}
