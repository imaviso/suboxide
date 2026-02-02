//! Annotation-related API handlers (star, unstar, getStarred2, scrobble, getNowPlaying, setRating, etc.)
#![allow(clippy::unused_async)]

use axum::extract::RawQuery;
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;
use crate::api::response::{error_response, ok_empty, ok_now_playing, ok_starred2};
use crate::models::music::{
    NowPlayingEntryResponse, NowPlayingResponse, Starred2Response, StarredAlbumID3Response,
    StarredArtistID3Response, StarredChildResponse,
};

/// Parse repeated query parameters from a query string.
/// Handles both single values and repeated parameters like `?id=1&id=2`.
fn parse_repeated_param(query: &str, param_name: &str) -> Vec<String> {
    let mut values = Vec::new();
    for part in query.split('&') {
        if let Some((key, value)) = part.split_once('=')
            && key == param_name
        {
            // URL decode the value
            values.push(
                urlencoding::decode(value)
                    .map_or_else(|_| value.to_string(), std::borrow::Cow::into_owned),
            );
        }
    }
    values
}

/// GET/POST /rest/star[.view]
///
/// Stars one or more artists, albums, or songs.
/// Supports multiple IDs via repeated parameters: `?id=1&id=2&albumId=3`
pub async fn star(RawQuery(query): RawQuery, auth: SubsonicAuth) -> impl IntoResponse {
    let query = query.unwrap_or_default();
    let user_id = auth.user.id;

    // Parse repeated parameters
    let song_ids = parse_repeated_param(&query, "id");
    let album_ids = parse_repeated_param(&query, "albumId");
    let artist_ids = parse_repeated_param(&query, "artistId");

    // Star artists
    for artist_id_str in &artist_ids {
        if let Ok(artist_id) = artist_id_str.parse::<i32>()
            && let Err(e) = auth.state.star_artist(user_id, artist_id)
        {
            tracing::warn!(artist_id = artist_id, error = %e, "Failed to star artist");
        }
    }

    // Star albums
    for album_id_str in &album_ids {
        if let Ok(album_id) = album_id_str.parse::<i32>()
            && let Err(e) = auth.state.star_album(user_id, album_id)
        {
            tracing::warn!(album_id = album_id, error = %e, "Failed to star album");
        }
    }

    // Star songs (id parameter)
    for song_id_str in &song_ids {
        if let Ok(song_id) = song_id_str.parse::<i32>()
            && let Err(e) = auth.state.star_song(user_id, song_id)
        {
            tracing::warn!(song_id = song_id, error = %e, "Failed to star song");
        }
    }

    ok_empty(auth.format)
}

/// GET/POST /rest/unstar[.view]
///
/// Unstars one or more artists, albums, or songs.
/// Supports multiple IDs via repeated parameters: `?id=1&id=2&albumId=3`
pub async fn unstar(RawQuery(query): RawQuery, auth: SubsonicAuth) -> impl IntoResponse {
    let query = query.unwrap_or_default();
    let user_id = auth.user.id;

    // Parse repeated parameters
    let song_ids = parse_repeated_param(&query, "id");
    let album_ids = parse_repeated_param(&query, "albumId");
    let artist_ids = parse_repeated_param(&query, "artistId");

    // Unstar artists
    for artist_id_str in &artist_ids {
        if let Ok(artist_id) = artist_id_str.parse::<i32>()
            && let Err(e) = auth.state.unstar_artist(user_id, artist_id)
        {
            tracing::warn!(artist_id = artist_id, error = %e, "Failed to unstar artist");
        }
    }

    // Unstar albums
    for album_id_str in &album_ids {
        if let Ok(album_id) = album_id_str.parse::<i32>()
            && let Err(e) = auth.state.unstar_album(user_id, album_id)
        {
            tracing::warn!(album_id = album_id, error = %e, "Failed to unstar album");
        }
    }

    // Unstar songs (id parameter)
    for song_id_str in &song_ids {
        if let Ok(song_id) = song_id_str.parse::<i32>()
            && let Err(e) = auth.state.unstar_song(user_id, song_id)
        {
            tracing::warn!(song_id = song_id, error = %e, "Failed to unstar song");
        }
    }

    ok_empty(auth.format)
}

/// GET/POST /rest/getStarred2[.view]
///
/// Returns all starred artists, albums, and songs for the current user.
/// Uses ID3 tags (artist/album/song structure).
pub async fn get_starred2(auth: SubsonicAuth) -> impl IntoResponse {
    let user_id = auth.user.id;

    // Get starred artists
    let starred_artists = auth.state.get_starred_artists(user_id);

    // Get album counts for all starred artists in a single batch query
    let artist_ids: Vec<i32> = starred_artists.iter().map(|(a, _)| a.id).collect();
    let album_counts = auth.state.get_artist_album_counts_batch(&artist_ids);

    let artists: Vec<StarredArtistID3Response> = starred_artists
        .iter()
        .map(|(artist, starred_at)| {
            let album_count = album_counts.get(&artist.id).copied().unwrap_or(0);
            StarredArtistID3Response::from_artist_and_starred(
                artist,
                Some(i32::try_from(album_count).unwrap_or(0)),
                starred_at,
            )
        })
        .collect();

    // Get starred albums
    let starred_albums = auth.state.get_starred_albums(user_id);
    let albums: Vec<StarredAlbumID3Response> = starred_albums
        .iter()
        .map(|(album, starred_at)| {
            StarredAlbumID3Response::from_album_and_starred(album, starred_at)
        })
        .collect();

    // Get starred songs
    let starred_songs = auth.state.get_starred_songs(user_id);
    let songs: Vec<StarredChildResponse> = starred_songs
        .iter()
        .map(|(song, starred_at)| StarredChildResponse::from_song_and_starred(song, starred_at))
        .collect();

    let response = Starred2Response {
        artists,
        albums,
        songs,
    };

    ok_starred2(auth.format, response)
}

/// GET/POST /rest/scrobble[.view]
///
/// Registers the local playback of one or more media files.
/// Typically used to notify the server about what is currently being played locally.
///
/// Parameters:
/// - `id` (required): The ID of the song being played (can be repeated)
/// - `time` (optional): Time in milliseconds since the media started playing (can be repeated, one per id)
/// - `submission` (optional): Whether this is a "scrobble" (true) or a "now playing" notification (false). Default true.
pub async fn scrobble(RawQuery(query): RawQuery, auth: SubsonicAuth) -> impl IntoResponse {
    let query = query.unwrap_or_default();
    let user_id = auth.user.id;

    // Parse parameters
    let song_ids = parse_repeated_param(&query, "id");
    let times = parse_repeated_param(&query, "time");

    // Parse submission parameter (default is true)
    let submission = parse_repeated_param(&query, "submission")
        .first()
        .is_none_or(|s| s != "false" && s != "0");

    // Get player_id from the client identifier
    let player_id = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    // Process each song ID
    for (i, song_id_str) in song_ids.iter().enumerate() {
        if let Ok(song_id) = song_id_str.parse::<i32>() {
            // Get the corresponding time if provided
            let time = times.get(i).and_then(|t| t.parse::<i64>().ok());

            // Register the scrobble
            if let Err(e) = auth.state.scrobble(user_id, song_id, time, submission) {
                tracing::warn!(song_id = song_id, error = %e, "Failed to scrobble song");
            }

            // If this is a "now playing" notification (submission=false), also update now playing
            if !submission && let Err(e) = auth.state.set_now_playing(user_id, song_id, player_id) {
                tracing::warn!(song_id = song_id, error = %e, "Failed to set now playing for song");
            }
        }
    }

    ok_empty(auth.format)
}

/// GET/POST /rest/getNowPlaying[.view]
///
/// Returns what is currently being played by all users.
pub async fn get_now_playing(auth: SubsonicAuth) -> impl IntoResponse {
    let entries = auth.state.get_now_playing();

    let entry_responses: Vec<NowPlayingEntryResponse> = entries
        .iter()
        .map(|entry| {
            NowPlayingEntryResponse::from_now_playing(
                &entry.song,
                entry.username.clone(),
                entry.minutes_ago,
                entry.player_id.clone(),
            )
        })
        .collect();

    let response = NowPlayingResponse {
        entries: entry_responses,
    };

    ok_now_playing(auth.format, response)
}

// ============================================================================
// Rating endpoints
// ============================================================================

/// Query parameters for setRating.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SetRatingParams {
    /// The ID of the item (song, album, or artist) to rate.
    pub id: Option<String>,
    /// The rating (0-5). 0 removes the rating.
    pub rating: Option<i32>,
}

/// GET/POST /rest/setRating[.view]
///
/// Sets the rating for a music file (song).
///
/// Parameters:
/// - `id` (required): The ID of the item to rate
/// - `rating` (required): The rating (0-5). 0 removes the rating.
pub async fn set_rating(
    axum::extract::Query(params): axum::extract::Query<SetRatingParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Validate required parameters
    let Some(id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    let rating = match params.rating {
        Some(r) if (0..=5).contains(&r) => r,
        Some(_) => {
            return error_response(
                auth.format,
                &ApiError::Generic("Rating must be between 0 and 5".into()),
            )
            .into_response();
        }
        None => {
            return error_response(auth.format, &ApiError::MissingParameter("rating".into()))
                .into_response();
        }
    };

    let user_id = auth.user.id;

    // Try to set rating (we default to song rating as that's most common)
    if let Err(e) = auth.state.set_song_rating(user_id, id, rating) {
        tracing::warn!(item_id = id, error = %e, "Failed to set rating for item");
        // Don't return an error - the API spec says this should succeed silently
    }

    ok_empty(auth.format).into_response()
}
