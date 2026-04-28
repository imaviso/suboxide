//! Annotation-related API handlers (star, unstar, getStarred2, scrobble, getNowPlaying, setRating, etc.)
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;

use crate::api::response::{SubsonicResponse, error_response};
use crate::api::services::saturating_i64_to_i32;
use crate::models::music::{
    NowPlayingEntryResponse, NowPlayingResponse, Starred2Response, StarredAlbumID3Response,
    StarredArtistID3Response, StarredChildResponse,
};

/// Query parameters for star/unstar.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_field_names,
    reason = "Subsonic API parameter names all end in 'Id'"
)]
pub struct StarParams {
    #[serde(rename = "artistId")]
    artist_id: Vec<i32>,
    #[serde(rename = "albumId")]
    album_id: Vec<i32>,
    #[serde(rename = "id")]
    song_id: Vec<i32>,
}

/// GET/POST /rest/star[.view]
///
/// Stars one or more artists, albums, or songs.
/// Supports multiple IDs via repeated parameters: `?id=1&id=2&albumId=3`
pub async fn star(
    axum::extract::Query(params): axum::extract::Query<StarParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let user_id = auth.user.id;

    for artist_id in &params.artist_id {
        if let Err(error) = auth.music().star_artist(user_id, *artist_id) {
            return error_response(auth.format, &ApiError::Generic(error.to_string()))
                .into_response();
        }
    }
    for album_id in &params.album_id {
        if let Err(error) = auth.music().star_album(user_id, *album_id) {
            return error_response(auth.format, &ApiError::Generic(error.to_string()))
                .into_response();
        }
    }
    for song_id in &params.song_id {
        if let Err(error) = auth.music().star_song(user_id, *song_id) {
            return error_response(auth.format, &ApiError::Generic(error.to_string()))
                .into_response();
        }
    }

    SubsonicResponse::empty(auth.format).into_response()
}

/// GET/POST /rest/unstar[.view]
///
/// Unstars one or more artists, albums, or songs.
/// Supports multiple IDs via repeated parameters: `?id=1&id=2&albumId=3`
pub async fn unstar(
    axum::extract::Query(params): axum::extract::Query<StarParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let user_id = auth.user.id;

    for artist_id in &params.artist_id {
        if let Err(error) = auth.music().unstar_artist(user_id, *artist_id) {
            return error_response(auth.format, &ApiError::Generic(error.to_string()))
                .into_response();
        }
    }
    for album_id in &params.album_id {
        if let Err(error) = auth.music().unstar_album(user_id, *album_id) {
            return error_response(auth.format, &ApiError::Generic(error.to_string()))
                .into_response();
        }
    }
    for song_id in &params.song_id {
        if let Err(error) = auth.music().unstar_song(user_id, *song_id) {
            return error_response(auth.format, &ApiError::Generic(error.to_string()))
                .into_response();
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

    let starred_artists = match auth.music().get_starred_artists(user_id) {
        Ok(v) => v,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };
    let artist_ids: Vec<i32> = starred_artists.iter().map(|(a, _)| a.id).collect();
    let album_counts = match auth.music().get_artist_album_counts_batch(&artist_ids) {
        Ok(v) => v,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
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

    let starred_albums = match auth.music().get_starred_albums(user_id) {
        Ok(v) => v,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };
    let albums: Vec<StarredAlbumID3Response> = starred_albums
        .iter()
        .map(|(album, starred_at)| {
            StarredAlbumID3Response::from_album_and_starred(album, starred_at)
        })
        .collect();

    let starred_songs = match auth.music().get_starred_songs(user_id) {
        Ok(v) => v,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
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

/// Query parameters for scrobble.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ScrobbleParams {
    #[serde(rename = "id")]
    song_id: Vec<i32>,
    time: Vec<i64>,
    submission: Option<String>,
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
pub async fn scrobble(
    axum::extract::Query(params): axum::extract::Query<ScrobbleParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let user_id = auth.user.id;

    let submission = params
        .submission
        .as_deref()
        .is_none_or(|s| s != "false" && s != "0");

    let player_id = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    for (i, song_id) in params.song_id.iter().enumerate() {
        let time = params.time.get(i).copied();

        if let Err(error) = auth.music().scrobble(user_id, *song_id, time, submission) {
            return error_response(auth.format, &ApiError::Generic(error.to_string()))
                .into_response();
        }

        if !submission
            && let Err(error) = auth.music().set_now_playing(user_id, *song_id, player_id)
        {
            return error_response(auth.format, &ApiError::Generic(error.to_string()))
                .into_response();
        }
    }

    SubsonicResponse::empty(auth.format).into_response()
}

/// GET/POST /rest/getNowPlaying[.view]
///
/// Returns what is currently being played by all users.
pub async fn get_now_playing(auth: SubsonicAuth) -> impl IntoResponse {
    let entries = match auth.music().get_now_playing() {
        Ok(v) => v,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
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

    match auth.music().set_song_rating(user_id, id, rating) {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(error) => {
            error_response(auth.format, &ApiError::Generic(error.to_string())).into_response()
        }
    }
}
