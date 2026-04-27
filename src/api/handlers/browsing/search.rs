//! Search handlers.

use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicAuth;
use crate::api::handlers::repo_result_or_response;
use crate::api::response::SubsonicResponse;
use crate::models::music::{
    AlbumID3Response, ArtistID3Response, ArtistResponse, ChildResponse, SearchMatch,
    SearchResult2Response, SearchResult3Response, SearchResultResponse, format_subsonic_datetime,
};

/// Query parameters for search3.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Search3Params {
    /// Search query.
    pub query: Option<String>,
    /// Maximum number of artists to return. Default 20.
    #[serde(rename = "artistCount")]
    pub artist_count: Option<i64>,
    /// Artist search result offset. Default 0.
    #[serde(rename = "artistOffset")]
    pub artist_offset: Option<i64>,
    /// Maximum number of albums to return. Default 20.
    #[serde(rename = "albumCount")]
    pub album_count: Option<i64>,
    /// Album search result offset. Default 0.
    #[serde(rename = "albumOffset")]
    pub album_offset: Option<i64>,
    /// Maximum number of songs to return. Default 20.
    #[serde(rename = "songCount")]
    pub song_count: Option<i64>,
    /// Song search result offset. Default 0.
    #[serde(rename = "songOffset")]
    pub song_offset: Option<i64>,
    /// Only return results from this music folder.
    #[serde(rename = "musicFolderId")]
    pub music_folder_id: Option<i32>,
}

/// GET/POST /rest/search3[.view]
///
/// Returns albums, artists and songs matching the given search criteria.
/// Supports paging through the result.
/// An empty query returns all results (up to the count limits).
pub async fn search3(
    axum::extract::Query(params): axum::extract::Query<Search3Params>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Empty query is allowed - it returns all results
    // Some clients send "" (quoted empty string) which we need to handle
    let raw_query = params.query.as_deref().unwrap_or("");
    let query = raw_query.trim_matches('"').trim();

    let artist_count = params.artist_count.unwrap_or(20).clamp(0, 500);
    let artist_offset = params.artist_offset.unwrap_or(0).max(0);
    let album_count = params.album_count.unwrap_or(20).clamp(0, 500);
    let album_offset = params.album_offset.unwrap_or(0).max(0);
    let song_count = params.song_count.unwrap_or(20).clamp(0, 500);
    let song_offset = params.song_offset.unwrap_or(0).max(0);

    let artists = match repo_result_or_response(
        auth.format,
        auth.state()
            .search_artists(query, artist_offset, artist_count),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };
    let albums = match repo_result_or_response(
        auth.format,
        auth.state().search_albums(query, album_offset, album_count),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };
    let songs = match repo_result_or_response(
        auth.format,
        auth.state().search_songs(query, song_offset, song_count),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };

    let user_id = auth.user.id;
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let album_ids: Vec<i32> = albums.iter().map(|a| a.id).collect();
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();

    let artist_album_counts = match repo_result_or_response(
        auth.format,
        auth.state().get_artist_album_counts_batch(&artist_ids),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };
    let starred_artists = match repo_result_or_response(
        auth.format,
        auth.state()
            .get_starred_at_for_artists_batch(user_id, &artist_ids),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };
    let starred_albums = match repo_result_or_response(
        auth.format,
        auth.state()
            .get_starred_at_for_albums_batch(user_id, &album_ids),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };
    let starred_songs = match repo_result_or_response(
        auth.format,
        auth.state()
            .get_starred_at_for_songs_batch(user_id, &song_ids),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };

    // Convert to response types with starred status from batch results
    let artist_responses: Vec<ArtistID3Response> = artists
        .iter()
        .map(|a| {
            let album_count = artist_album_counts.get(&a.id).copied().unwrap_or(0);
            let starred_at = starred_artists.get(&a.id);
            ArtistID3Response::from_artist_with_starred(
                a,
                Some(i32::try_from(album_count).unwrap_or(0)),
                starred_at,
            )
        })
        .collect();

    let album_responses: Vec<AlbumID3Response> = albums
        .iter()
        .map(|a| {
            let starred_at = starred_albums.get(&a.id);
            AlbumID3Response::from_album_with_starred(a, starred_at)
        })
        .collect();

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|s| {
            let starred_at = starred_songs.get(&s.id);
            ChildResponse::from_song_with_starred(s, starred_at)
        })
        .collect();

    let response = SearchResult3Response {
        artists: artist_responses,
        albums: album_responses,
        songs: song_responses,
    };

    SubsonicResponse::search_result3(auth.format, response).into_response()
}

/// Query parameters for search2.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Search2Params {
    /// Search query.
    pub query: Option<String>,
    /// Maximum number of artists to return. Default 20.
    #[serde(rename = "artistCount")]
    pub artist_count: Option<i64>,
    /// Artist search result offset. Default 0.
    #[serde(rename = "artistOffset")]
    pub artist_offset: Option<i64>,
    /// Maximum number of albums to return. Default 20.
    #[serde(rename = "albumCount")]
    pub album_count: Option<i64>,
    /// Album search result offset. Default 0.
    #[serde(rename = "albumOffset")]
    pub album_offset: Option<i64>,
    /// Maximum number of songs to return. Default 20.
    #[serde(rename = "songCount")]
    pub song_count: Option<i64>,
    /// Song search result offset. Default 0.
    #[serde(rename = "songOffset")]
    pub song_offset: Option<i64>,
    /// Only return results from this music folder.
    #[serde(rename = "musicFolderId")]
    pub music_folder_id: Option<i32>,
}

/// GET/POST /rest/search2[.view]
///
/// Returns albums, artists and songs matching the given search criteria (non-ID3).
pub async fn search2(
    axum::extract::Query(params): axum::extract::Query<Search2Params>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let raw_query = params.query.as_deref().unwrap_or("");
    let query = raw_query.trim_matches('"').trim();

    let artist_count = params.artist_count.unwrap_or(20).clamp(0, 500);
    let artist_offset = params.artist_offset.unwrap_or(0).max(0);
    let album_count = params.album_count.unwrap_or(20).clamp(0, 500);
    let album_offset = params.album_offset.unwrap_or(0).max(0);
    let song_count = params.song_count.unwrap_or(20).clamp(0, 500);
    let song_offset = params.song_offset.unwrap_or(0).max(0);

    let artists = match repo_result_or_response(
        auth.format,
        auth.state()
            .search_artists(query, artist_offset, artist_count),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };
    let albums = match repo_result_or_response(
        auth.format,
        auth.state().search_albums(query, album_offset, album_count),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };
    let songs = match repo_result_or_response(
        auth.format,
        auth.state().search_songs(query, song_offset, song_count),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };

    let user_id = auth.user.id;
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let album_ids: Vec<i32> = albums.iter().map(|a| a.id).collect();
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();

    let starred_artists = match repo_result_or_response(
        auth.format,
        auth.state()
            .get_starred_at_for_artists_batch(user_id, &artist_ids),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };
    let starred_albums = match repo_result_or_response(
        auth.format,
        auth.state()
            .get_starred_at_for_albums_batch(user_id, &album_ids),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };
    let starred_songs = match repo_result_or_response(
        auth.format,
        auth.state()
            .get_starred_at_for_songs_batch(user_id, &song_ids),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };

    // Convert to non-ID3 response types
    let artist_responses: Vec<ArtistResponse> = artists
        .iter()
        .map(|a| {
            let starred_at = starred_artists.get(&a.id);
            ArtistResponse::from_artist_with_starred(a, starred_at)
        })
        .collect();

    let album_responses: Vec<ChildResponse> = albums
        .iter()
        .map(|a| {
            let starred_at = starred_albums.get(&a.id);
            let mut response = ChildResponse::from_album_as_dir(a);
            response.starred = starred_at.map(format_subsonic_datetime);
            response
        })
        .collect();

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|s| {
            let starred_at = starred_songs.get(&s.id);
            ChildResponse::from_song_with_starred(s, starred_at)
        })
        .collect();

    let response = SearchResult2Response {
        artists: artist_responses,
        albums: album_responses,
        songs: song_responses,
    };

    SubsonicResponse::search_result2(auth.format, response).into_response()
}

/// Query parameters for legacy search.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchParams {
    /// Artist to search for.
    pub artist: Option<String>,
    /// Album to search for.
    pub album: Option<String>,
    /// Song title to search for.
    pub title: Option<String>,
    /// Searches all fields.
    pub any: Option<String>,
    /// Maximum number of results to return. Default 20.
    pub count: Option<i64>,
    /// Search result offset. Default 0.
    pub offset: Option<i64>,
    /// Only return matches that are newer than this timestamp.
    #[serde(rename = "newerThan")]
    pub newer_than: Option<i64>,
}

/// GET/POST /rest/search[.view]
///
/// Returns a listing of files matching the given search criteria (legacy).
pub async fn search(
    axum::extract::Query(params): axum::extract::Query<SearchParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let count = params.count.unwrap_or(20).clamp(0, 500);
    let offset = params.offset.unwrap_or(0).max(0);

    // Use 'any' field for general search, or combine artist/album/title
    let query = params
        .any
        .as_deref()
        .or(params.title.as_deref())
        .or(params.album.as_deref())
        .or(params.artist.as_deref())
        .unwrap_or("")
        .trim();

    let songs =
        match repo_result_or_response(auth.format, auth.state().search_songs(query, offset, count))
        {
            Ok(v) => v,
            Err(response) => return response,
        };

    let matches: Vec<SearchMatch> = songs.iter().map(SearchMatch::from).collect();
    #[expect(
        clippy::cast_possible_wrap,
        reason = "Legacy Subsonic search response requires signed totalHits"
    )]
    let total_hits = matches.len() as i64;

    let response = SearchResultResponse {
        offset,
        total_hits,
        matches,
    };

    SubsonicResponse::search_result(auth.format, response).into_response()
}
