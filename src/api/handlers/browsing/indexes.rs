//! Index and structure browsing handlers.

use std::collections::BTreeMap;

use axum::response::IntoResponse;

use crate::api::auth::SubsonicAuth;
use crate::api::response::{ok_artists, ok_indexes, ok_music_folders};
use crate::models::music::{
    ArtistID3Response, ArtistResponse, ArtistsID3Response, IndexID3Response, IndexResponse,
    IndexesResponse, MusicFolderResponse,
};

/// GET/POST /rest/getMusicFolders[.view]
///
/// Returns all configured top-level music folders.
pub async fn get_music_folders(auth: SubsonicAuth) -> impl IntoResponse {
    let folders = auth.state.get_music_folders();
    let responses: Vec<MusicFolderResponse> =
        folders.iter().map(MusicFolderResponse::from).collect();
    ok_music_folders(auth.format, responses)
}

/// GET/POST /rest/getIndexes[.view]
///
/// Returns an indexed structure of all artists.
/// This is used by older clients that use the folder-based browsing model.
pub async fn get_indexes(auth: SubsonicAuth) -> impl IntoResponse {
    let artists = auth.state.get_artists();
    let user_id = auth.user.id;

    // Get starred status for all artists in a single batch query
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let starred_map = auth
        .state
        .get_starred_at_for_artists_batch(user_id, &artist_ids);

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
    let last_modified = auth
        .state
        .get_artists_last_modified()
        .map_or(0, |dt| dt.and_utc().timestamp_millis());

    let response = IndexesResponse {
        ignored_articles: "The El La Los Las Le Les".to_string(),
        last_modified,
        indexes,
    };

    ok_indexes(auth.format, response)
}

/// GET/POST /rest/getArtists[.view]
///
/// Similar to getIndexes, but returns artists using ID3 tags.
/// This is the preferred endpoint for modern clients.
pub async fn get_artists(auth: SubsonicAuth) -> impl IntoResponse {
    let artists = auth.state.get_artists();
    let user_id = auth.user.id;

    // Get album counts for all artists in a single batch query
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let album_counts = auth.state.get_artist_album_counts_batch(&artist_ids);

    // Get starred status for all artists in a single batch query
    let starred_map = auth
        .state
        .get_starred_at_for_artists_batch(user_id, &artist_ids);

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
                Some(i32::try_from(album_count).unwrap_or(0)),
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

    ok_artists(auth.format, response)
}
