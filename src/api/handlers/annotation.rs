//! Annotation-related API handlers (star, unstar, getStarred2, scrobble, getNowPlaying, setRating, etc.)
use axum::extract::RawQuery;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::api::auth::{SubsonicAuth, saturating_i64_to_i32};
use crate::api::error::ApiError;
use crate::api::handlers::{repo_error_response, repo_result_or_response};
use crate::api::response::{Format, SubsonicResponse, error_response};
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

#[expect(
    clippy::result_large_err,
    reason = "Err variant is axum Response used for immediate early-return"
)]
fn parse_i32_list(format: Format, values: &[String], param: &str) -> Result<Vec<i32>, Response> {
    let mut ids = Vec::with_capacity(values.len());
    for value in values {
        match value.parse::<i32>() {
            Ok(id) => ids.push(id),
            Err(_) => {
                return Err(error_response(
                    format,
                    &ApiError::Generic(format!("Invalid {param}: {value}")),
                )
                .into_response());
            }
        }
    }
    Ok(ids)
}

/// GET/POST /rest/star[.view]
///
/// Stars one or more artists, albums, or songs.
/// Supports multiple IDs via repeated parameters: `?id=1&id=2&albumId=3`
pub async fn star(RawQuery(query): RawQuery, auth: SubsonicAuth) -> impl IntoResponse {
    let query = query.unwrap_or_default();
    let user_id = auth.user.id;

    let artist_id_strs = parse_repeated_param(&query, "artistId");
    let album_id_strs = parse_repeated_param(&query, "albumId");
    let song_id_strs = parse_repeated_param(&query, "id");

    let artist_ids = match parse_i32_list(auth.format, &artist_id_strs, "artistId") {
        Ok(ids) => ids,
        Err(response) => return response,
    };
    let album_ids = match parse_i32_list(auth.format, &album_id_strs, "albumId") {
        Ok(ids) => ids,
        Err(response) => return response,
    };
    let song_ids = match parse_i32_list(auth.format, &song_id_strs, "id") {
        Ok(ids) => ids,
        Err(response) => return response,
    };

    for artist_id in &artist_ids {
        if let Err(error) = auth.state().star_artist(user_id, *artist_id) {
            return repo_error_response(auth.format, error);
        }
    }
    for album_id in &album_ids {
        if let Err(error) = auth.state().star_album(user_id, *album_id) {
            return repo_error_response(auth.format, error);
        }
    }
    for song_id in &song_ids {
        if let Err(error) = auth.state().star_song(user_id, *song_id) {
            return repo_error_response(auth.format, error);
        }
    }

    SubsonicResponse::empty(auth.format).into_response()
}

/// GET/POST /rest/unstar[.view]
///
/// Unstars one or more artists, albums, or songs.
/// Supports multiple IDs via repeated parameters: `?id=1&id=2&albumId=3`
pub async fn unstar(RawQuery(query): RawQuery, auth: SubsonicAuth) -> impl IntoResponse {
    let query = query.unwrap_or_default();
    let user_id = auth.user.id;

    let artist_id_strs = parse_repeated_param(&query, "artistId");
    let album_id_strs = parse_repeated_param(&query, "albumId");
    let song_id_strs = parse_repeated_param(&query, "id");

    let artist_ids = match parse_i32_list(auth.format, &artist_id_strs, "artistId") {
        Ok(ids) => ids,
        Err(response) => return response,
    };
    let album_ids = match parse_i32_list(auth.format, &album_id_strs, "albumId") {
        Ok(ids) => ids,
        Err(response) => return response,
    };
    let song_ids = match parse_i32_list(auth.format, &song_id_strs, "id") {
        Ok(ids) => ids,
        Err(response) => return response,
    };

    for artist_id in &artist_ids {
        if let Err(error) = auth.state().unstar_artist(user_id, *artist_id) {
            return repo_error_response(auth.format, error);
        }
    }
    for album_id in &album_ids {
        if let Err(error) = auth.state().unstar_album(user_id, *album_id) {
            return repo_error_response(auth.format, error);
        }
    }
    for song_id in &song_ids {
        if let Err(error) = auth.state().unstar_song(user_id, *song_id) {
            return repo_error_response(auth.format, error);
        }
    }

    SubsonicResponse::empty(auth.format).into_response()
}

/// GET/POST /rest/getStarred2[.view]
///
/// Returns all starred artists, albums, and songs for the current user.
/// Uses ID3 tags (artist/album/song structure).
pub async fn get_starred2(auth: SubsonicAuth) -> impl IntoResponse {
    let user_id = auth.user.id;

    let starred_artists =
        match repo_result_or_response(auth.format, auth.state().get_starred_artists(user_id)) {
            Ok(v) => v,
            Err(response) => return response,
        };
    let artist_ids: Vec<i32> = starred_artists.iter().map(|(a, _)| a.id).collect();
    let album_counts = match repo_result_or_response(
        auth.format,
        auth.state().get_artist_album_counts_batch(&artist_ids),
    ) {
        Ok(v) => v,
        Err(response) => return response,
    };

    let artists: Vec<StarredArtistID3Response> = starred_artists
        .iter()
        .map(|(artist, starred_at)| {
            let album_count = album_counts.get(&artist.id).copied().unwrap_or(0);
            StarredArtistID3Response::from_artist_and_starred(
                artist,
                Some(saturating_i64_to_i32(album_count)),
                starred_at,
            )
        })
        .collect();

    let starred_albums =
        match repo_result_or_response(auth.format, auth.state().get_starred_albums(user_id)) {
            Ok(v) => v,
            Err(response) => return response,
        };
    let albums: Vec<StarredAlbumID3Response> = starred_albums
        .iter()
        .map(|(album, starred_at)| {
            StarredAlbumID3Response::from_album_and_starred(album, starred_at)
        })
        .collect();

    let starred_songs =
        match repo_result_or_response(auth.format, auth.state().get_starred_songs(user_id)) {
            Ok(v) => v,
            Err(response) => return response,
        };
    let songs: Vec<StarredChildResponse> = starred_songs
        .iter()
        .map(|(song, starred_at)| StarredChildResponse::from_song_and_starred(song, starred_at))
        .collect();

    let response = Starred2Response {
        artists,
        albums,
        songs,
    };
    SubsonicResponse::starred2(auth.format, response).into_response()
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

    let song_id_strs = parse_repeated_param(&query, "id");
    let times = parse_repeated_param(&query, "time");

    let song_ids = match parse_i32_list(auth.format, &song_id_strs, "id") {
        Ok(ids) => ids,
        Err(response) => return response,
    };

    let submission = parse_repeated_param(&query, "submission")
        .first()
        .is_none_or(|s| s != "false" && s != "0");

    let player_id = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    for (i, song_id) in song_ids.iter().enumerate() {
        let time = times.get(i).and_then(|t| t.parse::<i64>().ok());

        if let Err(error) = auth.state().scrobble(user_id, *song_id, time, submission) {
            return repo_error_response(auth.format, error);
        }

        if !submission
            && let Err(error) = auth.state().set_now_playing(user_id, *song_id, player_id)
        {
            return repo_error_response(auth.format, error);
        }
    }

    SubsonicResponse::empty(auth.format).into_response()
}

/// GET/POST /rest/getNowPlaying[.view]
///
/// Returns what is currently being played by all users.
pub async fn get_now_playing(auth: SubsonicAuth) -> impl IntoResponse {
    let entries = match repo_result_or_response(auth.format, auth.state().get_now_playing()) {
        Ok(v) => v,
        Err(response) => return response,
    };

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

    SubsonicResponse::now_playing(auth.format, response).into_response()
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
    let Some(id_str) = params.id.as_ref() else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };
    let Ok(id) = id_str.parse::<i32>() else {
        return error_response(
            auth.format,
            &ApiError::Generic(format!("Invalid id: {id_str}")),
        )
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

    match auth.state().set_song_rating(user_id, id, rating) {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(error) => repo_error_response(auth.format, error),
    }
}
