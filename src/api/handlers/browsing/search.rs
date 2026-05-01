//! Search handlers.

use std::collections::HashMap;

use axum::response::{IntoResponse, Response};
use chrono::NaiveDateTime;
use serde::Deserialize;

use crate::api::auth::SubsonicContext;
use crate::api::error::ApiError;
use crate::api::handlers::util;
use crate::api::response::SubsonicResponse;
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

struct SearchStars {
    artists: HashMap<i32, NaiveDateTime>,
    albums: HashMap<i32, NaiveDateTime>,
    songs: HashMap<i32, NaiveDateTime>,
}

fn search_limits(params: &SearchParamsV2) -> SearchLimits<'_> {
    let raw_query = params.query.as_deref().unwrap_or("").trim();
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

fn search_stars(auth: &SubsonicContext, data: &SearchData) -> Result<SearchStars, Box<Response>> {
    let artist_ids: Vec<i32> = data.artists.iter().map(|artist| artist.id).collect();
    let album_ids: Vec<i32> = data.albums.iter().map(|album| album.id).collect();
    let song_ids: Vec<i32> = data.songs.iter().map(|song| song.id).collect();

    let artists = auth
        .music()
        .get_starred_at_for_artists_batch(auth.user.id, &artist_ids)
        .map_err(|error| Box::new(util::service_error(auth, error)))?;
    let albums = auth
        .music()
        .get_starred_at_for_albums_batch(auth.user.id, &album_ids)
        .map_err(|error| Box::new(util::service_error(auth, error)))?;
    let songs = auth
        .music()
        .get_starred_at_for_songs_batch(auth.user.id, &song_ids)
        .map_err(|error| Box::new(util::service_error(auth, error)))?;

    Ok(SearchStars {
        artists,
        albums,
        songs,
    })
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
    let data = match search_data(&auth, &limits) {
        Ok(data) => data,
        Err(error) => return util::api_error(&auth, &error),
    };

    let artist_ids: Vec<i32> = data.artists.iter().map(|artist| artist.id).collect();

    let artist_album_counts = match auth.music().get_artist_album_counts_batch(&artist_ids) {
        Ok(counts) => counts,
        Err(error) => {
            return util::service_error(&auth, error);
        }
    };
    let stars = match search_stars(&auth, &data) {
        Ok(stars) => stars,
        Err(response) => return *response,
    };

    // Convert to response types with starred status from batch results
    let artist_responses: Vec<ArtistID3Response> = data
        .artists
        .iter()
        .map(|a| {
            let album_count = artist_album_counts.get(&a.id).copied().unwrap_or(0);
            let starred_at = stars.artists.get(&a.id);
            ArtistID3Response::from_artist_with_starred(
                a,
                Some(saturating_i64_to_i32(album_count)),
                starred_at,
            )
        })
        .collect();

    let album_responses: Vec<AlbumID3Response> = data
        .albums
        .iter()
        .map(|a| {
            let starred_at = stars.albums.get(&a.id);
            AlbumID3Response::from_album_with_starred(a, starred_at)
        })
        .collect();

    let song_responses: Vec<ChildResponse> = data
        .songs
        .iter()
        .map(|s| {
            let starred_at = stars.songs.get(&s.id);
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
    let data = match search_data(&auth, &limits) {
        Ok(data) => data,
        Err(error) => return util::api_error(&auth, &error),
    };

    let stars = match search_stars(&auth, &data) {
        Ok(stars) => stars,
        Err(response) => return *response,
    };

    // Convert to non-ID3 response types
    let artist_responses: Vec<ArtistResponse> = data
        .artists
        .iter()
        .map(|a| {
            let starred_at = stars.artists.get(&a.id);
            ArtistResponse::from_artist_with_starred(a, starred_at)
        })
        .collect();

    let album_responses: Vec<ChildResponse> = data
        .albums
        .iter()
        .map(|a| {
            let starred_at = stars.albums.get(&a.id);
            let mut response = ChildResponse::from_album_as_dir(a);
            response.starred = starred_at.map(format_subsonic_datetime);
            response
        })
        .collect();

    let song_responses: Vec<ChildResponse> = data
        .songs
        .iter()
        .map(|s| {
            let starred_at = stars.songs.get(&s.id);
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
            return util::service_error(&auth, error);
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

#[cfg(test)]
mod tests {
    use super::{SearchParamsV2, search_limits};

    #[test]
    fn search_limits_trim_wrapping_quotes_and_outer_whitespace() {
        let params = SearchParamsV2 {
            query: Some(" \" Miles Davis \" ".to_string()),
            ..SearchParamsV2::default()
        };

        assert_eq!(search_limits(&params).query, "Miles Davis");
    }

    #[test]
    fn search_limits_clamp_counts_and_offsets_deterministically() {
        let params = SearchParamsV2 {
            artist_count: Some(999),
            artist_offset: Some(-8),
            album_count: Some(-4),
            album_offset: Some(-1),
            song_count: Some(501),
            song_offset: Some(12),
            ..SearchParamsV2::default()
        };

        let limits = search_limits(&params);

        assert_eq!(limits.artist_count, 500);
        assert_eq!(limits.artist_offset, 0);
        assert_eq!(limits.album_count, 0);
        assert_eq!(limits.album_offset, 0);
        assert_eq!(limits.song_count, 500);
        assert_eq!(limits.song_offset, 12);
    }
}
