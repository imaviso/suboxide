//! Play queue API handlers (getPlayQueue, savePlayQueue, getPlayQueueByIndex, savePlayQueueByIndex)
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicContext;
use crate::api::error::ApiError;
use crate::api::response::{SubsonicResponse, error_response};
use crate::db::PlayQueue;
use crate::models::music::{
    ChildResponse, PlayQueueByIndexResponse, PlayQueueResponse, format_subsonic_datetime,
};

fn play_queue_entries(
    auth: &SubsonicContext,
    play_queue: &PlayQueue,
) -> Result<Vec<ChildResponse>, ApiError> {
    let song_ids: Vec<i32> = play_queue.songs.iter().map(|song| song.id).collect();
    let starred_map = auth
        .music()
        .get_starred_at_for_songs_batch(auth.user.id, &song_ids)
        .map_err(|error| ApiError::Generic(error.to_string()))?;

    Ok(play_queue
        .songs
        .iter()
        .map(|song| {
            let starred_at = starred_map.get(&song.id);
            ChildResponse::from_song_with_starred(song, starred_at)
        })
        .collect())
}

fn play_queue_response(
    auth: &SubsonicContext,
    play_queue: PlayQueue,
) -> Result<PlayQueueResponse, ApiError> {
    let entries = play_queue_entries(auth, &play_queue)?;
    Ok(PlayQueueResponse {
        current: play_queue
            .current_song
            .as_ref()
            .map(|song| song.id.to_string()),
        position: play_queue.position,
        username: play_queue.username,
        changed: format_subsonic_datetime(&play_queue.changed_at),
        changed_by: play_queue.changed_by,
        entries,
    })
}

fn play_queue_by_index_response(
    auth: &SubsonicContext,
    play_queue: PlayQueue,
) -> Result<PlayQueueByIndexResponse, ApiError> {
    let entries = play_queue_entries(auth, &play_queue)?;
    let current_index = play_queue.current_song.as_ref().and_then(|current_song| {
        play_queue
            .songs
            .iter()
            .position(|song| song.id == current_song.id)
            .and_then(|index| i32::try_from(index).ok())
    });

    Ok(PlayQueueByIndexResponse {
        current_index,
        position: play_queue.position,
        username: play_queue.username,
        changed: format_subsonic_datetime(&play_queue.changed_at),
        changed_by: play_queue.changed_by,
        entries,
    })
}

/// GET/POST /rest/getPlayQueue[.view]
///
/// Returns the current play queue for the user.
pub async fn get_play_queue(auth: SubsonicContext) -> impl IntoResponse {
    let user_id = auth.user.id;
    let username = &auth.user.username;

    match auth.music().get_play_queue(user_id, username) {
        Ok(Some(play_queue)) => {
            let response = match play_queue_response(&auth, play_queue) {
                Ok(response) => response,
                Err(error) => return error_response(auth.format, &error).into_response(),
            };

            SubsonicResponse::play_queue(auth.format, response).into_response()
        }
        Ok(None) => {
            let response = PlayQueueResponse {
                current: None,
                position: None,
                username: username.clone(),
                changed: format_subsonic_datetime(&chrono::Utc::now().naive_utc()),
                changed_by: None,
                entries: vec![],
            };

            SubsonicResponse::play_queue(auth.format, response).into_response()
        }
        Err(e) => error_response(auth.format, &ApiError::Generic(e.to_string())).into_response(),
    }
}

/// Query parameters for savePlayQueue.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SavePlayQueueParams {
    /// IDs of songs in the play queue (can be repeated).
    #[serde(rename = "id")]
    song_id: Vec<i32>,
    /// The ID of the currently playing song.
    current: Option<i32>,
    /// Position in milliseconds within the currently playing song.
    position: Option<i64>,
}

/// GET/POST /rest/savePlayQueue[.view]
///
/// Saves the current play queue for the user.
///
/// Parameters:
/// - `id`: ID of a song in the play queue (can be repeated to define the entire queue)
/// - `current`: The ID of the currently playing song
/// - `position`: Position in milliseconds within the currently playing song
pub async fn save_play_queue(
    axum::extract::Query(params): axum::extract::Query<SavePlayQueueParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let user_id = auth.user.id;
    let song_ids = params.song_id;

    let changed_by = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    match auth.music().save_play_queue(
        user_id,
        &song_ids,
        params.current,
        params.position,
        changed_by,
    ) {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(e) => error_response(auth.format, &ApiError::Generic(e.to_string())).into_response(),
    }
}

/// GET/POST /rest/getPlayQueueByIndex[.view]
///
/// Returns the current play queue for the user using queue index instead of song ID.
/// This is an `OpenSubsonic` extension.
pub async fn get_play_queue_by_index(auth: SubsonicContext) -> impl IntoResponse {
    let user_id = auth.user.id;
    let username = &auth.user.username;

    match auth.music().get_play_queue(user_id, username) {
        Ok(Some(play_queue)) => {
            let response = match play_queue_by_index_response(&auth, play_queue) {
                Ok(response) => response,
                Err(error) => return error_response(auth.format, &error).into_response(),
            };

            SubsonicResponse::play_queue_by_index(auth.format, response).into_response()
        }
        Ok(None) => {
            let response = PlayQueueByIndexResponse {
                current_index: None,
                position: None,
                username: username.clone(),
                changed: format_subsonic_datetime(&chrono::Utc::now().naive_utc()),
                changed_by: None,
                entries: vec![],
            };

            SubsonicResponse::play_queue_by_index(auth.format, response).into_response()
        }
        Err(e) => error_response(auth.format, &ApiError::Generic(e.to_string())).into_response(),
    }
}

/// Query parameters for savePlayQueueByIndex.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SavePlayQueueByIndexParams {
    /// IDs of songs in the play queue (can be repeated).
    #[serde(rename = "id")]
    song_id: Vec<i32>,
    /// The index of the currently playing song (0-based).
    #[serde(rename = "currentIndex")]
    current_index: Option<usize>,
    /// Position in milliseconds within the currently playing song.
    position: Option<i64>,
}

/// GET/POST /rest/savePlayQueueByIndex[.view]
///
/// Saves the current play queue for the user using queue index instead of song ID.
/// This is an `OpenSubsonic` extension.
///
/// Parameters:
/// - `id`: ID of a song in the play queue (can be repeated to define the entire queue)
/// - `currentIndex`: The index of the currently playing song (0-based)
/// - `position`: Position in milliseconds within the currently playing song
pub async fn save_play_queue_by_index(
    axum::extract::Query(params): axum::extract::Query<SavePlayQueueByIndexParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let user_id = auth.user.id;
    let song_ids = params.song_id;

    let current_song_id = params
        .current_index
        .and_then(|idx| song_ids.get(idx).copied());

    let changed_by = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    match auth.music().save_play_queue(
        user_id,
        &song_ids,
        current_song_id,
        params.position,
        changed_by,
    ) {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(e) => error_response(auth.format, &ApiError::Generic(e.to_string())).into_response(),
    }
}
