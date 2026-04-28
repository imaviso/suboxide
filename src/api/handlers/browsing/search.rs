//! Search handlers.

use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicContext;
use crate::api::error::ApiError;
use crate::api::response::{SubsonicResponse, error_response};
use crate::models::music::{
    AlbumID3Response, ArtistID3Response, ArtistResponse, ChildResponse, SearchMatch,
    SearchResult2Response, SearchResult3Response, SearchResultResponse, format_subsonic_datetime,
    saturating_i64_to_i32,
};

struct SearchLimits<'a> {
    query: &'a str,
    artist_count: i64,
    artist_offset: i64,
    album_count: i64,
    album_offset: i64,
    song_count: i64,
    song_offset: i64,
}

struct SearchData {
    artists: Vec<crate::models::music::Artist>,
    albums: Vec<crate::models::music::Album>,
    songs: Vec<crate::models::music::Song>,
}

fn search_limits(params: &SearchParamsV2) -> SearchLimits<'_> {
    let raw_query = params.query.as_deref().unwrap_or("");
    SearchLimits {
        query: raw_query.trim_matches('"').trim(),
        artist_count: params.artist_count.unwrap_or(20).clamp(0, 500),
        artist_offset: params.artist_offset.unwrap_or(0).max(0),
        album_count: params.album_count.unwrap_or(20).clamp(0, 500),
        album_offset: params.album_offset.unwrap_or(0).max(0),
        song_count: params.song_count.unwrap_or(20).clamp(0, 500),
        song_offset: params.song_offset.unwrap_or(0).max(0),
    }
}

fn search_data(auth: &SubsonicContext, limits: &SearchLimits<'_>) -> Result<SearchData, ApiError> {
    let artists = auth
        .music()
        .search_artists(limits.query, limits.artist_offset, limits.artist_count)
        .map_err(|e| ApiError::Generic(e.to_string()))?;
    let albums = auth
        .music()
        .search_albums(limits.query, limits.album_offset, limits.album_count)
        .map_err(|e| ApiError::Generic(e.to_string()))?;
    let songs = auth
        .music()
        .search_songs(limits.query, limits.song_offset, limits.song_count)
        .map_err(|e| ApiError::Generic(e.to_string()))?;

    Ok(SearchData {
        artists,
        albums,
        songs,
    })
}

fn api_error_response(
    format: crate::api::response::Format,
    error: &ApiError,
) -> axum::response::Response {
    error_response(format, error).into_response()
}

/// Query parameters for search3/search2.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchParamsV2 {
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<SearchParamsV2>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let limits = search_limits(&params);
    let SearchData {
        artists,
        albums,
        songs,
    } = match search_data(&auth, &limits) {
        Ok(data) => data,
        Err(error) => return api_error_response(auth.format, &error),
    };

    let user_id = auth.user.id;
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let album_ids: Vec<i32> = albums.iter().map(|a| a.id).collect();
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();

    let artist_album_counts = match auth.music().get_artist_album_counts_batch(&artist_ids) {
        Ok(counts) => counts,
        Err(error) => {
            return api_error_response(auth.format, &ApiError::Generic(error.to_string()));
        }
    };
    let starred_artists = match auth
        .music()
        .get_starred_at_for_artists_batch(user_id, &artist_ids)
    {
        Ok(starred) => starred,
        Err(error) => {
            return api_error_response(auth.format, &ApiError::Generic(error.to_string()));
        }
    };
    let starred_albums = match auth
        .music()
        .get_starred_at_for_albums_batch(user_id, &album_ids)
    {
        Ok(starred) => starred,
        Err(error) => {
            return api_error_response(auth.format, &ApiError::Generic(error.to_string()));
        }
    };
    let starred_songs = match auth
        .music()
        .get_starred_at_for_songs_batch(user_id, &song_ids)
    {
        Ok(starred) => starred,
        Err(error) => {
            return api_error_response(auth.format, &ApiError::Generic(error.to_string()));
        }
    };

    // Convert to response types with starred status from batch results
    let artist_responses: Vec<ArtistID3Response> = artists
        .iter()
        .map(|a| {
            let album_count = artist_album_counts.get(&a.id).copied().unwrap_or(0);
            let starred_at = starred_artists.get(&a.id);
            ArtistID3Response::from_artist_with_starred(
                a,
                Some(saturating_i64_to_i32(album_count)),
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

/// GET/POST /rest/search2[.view]
///
/// Returns albums, artists and songs matching the given search criteria (non-ID3).
pub async fn search2(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<SearchParamsV2>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let limits = search_limits(&params);
    let SearchData {
        artists,
        albums,
        songs,
    } = match search_data(&auth, &limits) {
        Ok(data) => data,
        Err(error) => return api_error_response(auth.format, &error),
    };

    let user_id = auth.user.id;
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let album_ids: Vec<i32> = albums.iter().map(|a| a.id).collect();
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();

    let starred_artists = match auth
        .music()
        .get_starred_at_for_artists_batch(user_id, &artist_ids)
    {
        Ok(starred) => starred,
        Err(error) => {
            return api_error_response(auth.format, &ApiError::Generic(error.to_string()));
        }
    };
    let starred_albums = match auth
        .music()
        .get_starred_at_for_albums_batch(user_id, &album_ids)
    {
        Ok(starred) => starred,
        Err(error) => {
            return api_error_response(auth.format, &ApiError::Generic(error.to_string()));
        }
    };
    let starred_songs = match auth
        .music()
        .get_starred_at_for_songs_batch(user_id, &song_ids)
    {
        Ok(starred) => starred,
        Err(error) => {
            return api_error_response(auth.format, &ApiError::Generic(error.to_string()));
        }
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<SearchParams>,
    auth: SubsonicContext,
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

    let songs = match auth.music().search_songs(query, offset, count) {
        Ok(songs) => songs,
        Err(error) => {
            return api_error_response(auth.format, &ApiError::Generic(error.to_string()));
        }
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
