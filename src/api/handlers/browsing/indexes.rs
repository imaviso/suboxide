//! Index and structure browsing handlers.

use std::collections::BTreeMap;

use axum::response::IntoResponse;

use crate::api::auth::SubsonicAuth;
use crate::api::handlers::repo_result_or_response;
use crate::api::response::SubsonicResponse;
use crate::models::music::{
    ArtistID3Response, ArtistResponse, ArtistsID3Response, IndexID3Response, IndexResponse,
    IndexesResponse, MusicFolderResponse,
};

#[expect(
    clippy::cast_possible_truncation,
    reason = "Subsonic album counts are bounded to signed 32-bit fields"
)]
fn saturating_i64_to_i32(value: i64) -> i32 {
    if value > i64::from(i32::MAX) {
        i32::MAX
    } else if value < i64::from(i32::MIN) {
        i32::MIN
    } else {
        value as i32
    }
}

/// GET/POST /rest/getMusicFolders[.view]
///
/// Returns all configured top-level music folders.
pub async fn get_music_folders(auth: SubsonicAuth) -> impl IntoResponse {
    let folders = match repo_result_or_response(auth.format, auth.music().get_music_folders()) {
        Ok(folders) => folders,
        Err(response) => return response,
    };
    let responses: Vec<MusicFolderResponse> =
        folders.iter().map(MusicFolderResponse::from).collect();
    SubsonicResponse::music_folders(auth.format, responses).into_response()
}

/// GET/POST /rest/getIndexes[.view]
///
/// Returns an indexed structure of all artists.
/// This is used by older clients that use the folder-based browsing model.
pub async fn get_indexes(auth: SubsonicAuth) -> impl IntoResponse {
    let artists = match repo_result_or_response(auth.format, auth.music().get_artists()) {
        Ok(artists) => artists,
        Err(response) => return response,
    };
    let user_id = auth.user.id;

    // Get starred status for all artists in a single batch query
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let starred_map = match repo_result_or_response(
        auth.format,
        auth.music()
            .get_starred_at_for_artists_batch(user_id, &artist_ids),
    ) {
        Ok(starred_map) => starred_map,
        Err(response) => return response,
    };

    // Group artists by first letter
    let mut index_map: BTreeMap<String, Vec<ArtistResponse>> = BTreeMap::new();

    for artist in &artists {
        let first_char = artist
            .sort_name
            .as_ref()
            .unwrap_or(&artist.name)
            .chars()
            .next()
            .unwrap_or('#')
            .to_uppercase()
            .next()
            .unwrap_or('#');

        let key = if first_char.is_alphabetic() {
            first_char.to_string()
        } else {
            "#".to_string()
        };

        let starred_at = starred_map.get(&artist.id);

        index_map
            .entry(key)
            .or_default()
            .push(ArtistResponse::from_artist_with_starred(artist, starred_at));
    }

    // Convert to response format
    let indexes: Vec<IndexResponse> = index_map
        .into_iter()
        .map(|(name, artists)| IndexResponse { name, artists })
        .collect();

    // Get last modified time (using current timestamp for now)
    let last_modified =
        match repo_result_or_response(auth.format, auth.music().get_artists_last_modified()) {
            Ok(value) => value.map_or(0, |dt| dt.and_utc().timestamp_millis()),
            Err(response) => return response,
        };

    let response = IndexesResponse {
        ignored_articles: "The El La Los Las Le Les".to_string(),
        last_modified,
        indexes,
    };

    SubsonicResponse::indexes(auth.format, response).into_response()
}

/// GET/POST /rest/getArtists[.view]
///
/// Similar to getIndexes, but returns artists using ID3 tags.
/// This is the preferred endpoint for modern clients.
pub async fn get_artists(auth: SubsonicAuth) -> impl IntoResponse {
    let artists = match repo_result_or_response(auth.format, auth.music().get_artists()) {
        Ok(artists) => artists,
        Err(response) => return response,
    };
    let user_id = auth.user.id;

    // Get album counts for all artists in a single batch query
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let album_counts = match repo_result_or_response(
        auth.format,
        auth.music().get_artist_album_counts_batch(&artist_ids),
    ) {
        Ok(album_counts) => album_counts,
        Err(response) => return response,
    };

    // Get starred status for all artists in a single batch query
    let starred_map = match repo_result_or_response(
        auth.format,
        auth.music()
            .get_starred_at_for_artists_batch(user_id, &artist_ids),
    ) {
        Ok(starred_map) => starred_map,
        Err(response) => return response,
    };

    // Group artists by first letter
    let mut index_map: BTreeMap<String, Vec<ArtistID3Response>> = BTreeMap::new();

    for artist in &artists {
        let first_char = artist
            .sort_name
            .as_ref()
            .unwrap_or(&artist.name)
            .chars()
            .next()
            .unwrap_or('#')
            .to_uppercase()
            .next()
            .unwrap_or('#');

        let key = if first_char.is_alphabetic() {
            first_char.to_string()
        } else {
            "#".to_string()
        };

        // Get album count and starred status from batch results
        let album_count = album_counts.get(&artist.id).copied().unwrap_or(0);
        let starred_at = starred_map.get(&artist.id);

        index_map
            .entry(key)
            .or_default()
            .push(ArtistID3Response::from_artist_with_starred(
                artist,
                Some(saturating_i64_to_i32(album_count)),
                starred_at,
            ));
    }

    // Convert to response format
    let indexes: Vec<IndexID3Response> = index_map
        .into_iter()
        .map(|(name, artists)| IndexID3Response { name, artists })
        .collect();

    let response = ArtistsID3Response {
        ignored_articles: "The El La Los Las Le Les".to_string(),
        indexes,
    };

    SubsonicResponse::artists(auth.format, response).into_response()
}
