//! Play queue API handlers (getPlayQueue, savePlayQueue, getPlayQueueByIndex, savePlayQueueByIndex)
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicContext;
use crate::api::error::ApiError;
use crate::api::handlers::util;
use crate::api::response::SubsonicResponse;
use crate::db::PlayQueue;
use crate::models::music::{
    ChildResponse, PlayQueueByIndexResponse, PlayQueueResponse, format_subsonic_datetime,
    song_api_id,
};

fn play_queue_entries(
    auth: &SubsonicContext,
    play_queue: &PlayQueue,
) -> Result<Vec<ChildResponse>, ApiError> {
    auth.music()
        .annotate_songs_for_user(auth.user.id, play_queue.songs.clone())
        .map(util::annotate_songs)
        .map_err(ApiError::from)
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
            .map(|song| song_api_id(song.id)),
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
                Err(error) => return util::api_error(&auth, &error),
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
        Err(e) => util::repo_error(&auth, e),
    }
}

/// Query parameters for savePlayQueue.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SavePlayQueueParams {
    /// IDs of songs in the play queue (can be repeated).
    #[serde(rename = "id")]
    song_id: Vec<String>,
    /// The ID of the currently playing song.
    current: Option<String>,
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<SavePlayQueueParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let user_id = auth.user.id;
    let song_ids = match util::parse_song_ids(&auth, &params.song_id, "id") {
        Ok(ids) => ids,
        Err(response) => return *response,
    };
    let current = match params.current.as_deref() {
        Some(current) => match crate::models::music::EntityId::parse_song(current) {
            Some(id) => Some(id),
            None => return util::service_error(&auth, format!("Invalid current: {current}")),
        },
        None => None,
    };

    let changed_by = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    match auth
        .music()
        .save_play_queue(user_id, &song_ids, current, params.position, changed_by)
    {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(e) => util::repo_error(&auth, e),
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
                Err(error) => return util::api_error(&auth, &error),
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
        Err(e) => util::repo_error(&auth, e),
    }
}

/// Query parameters for savePlayQueueByIndex.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SavePlayQueueByIndexParams {
    /// IDs of songs in the play queue (can be repeated).
    #[serde(rename = "id")]
    song_id: Vec<String>,
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<
        SavePlayQueueByIndexParams,
    >,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let user_id = auth.user.id;
    let song_ids = match util::parse_song_ids(&auth, &params.song_id, "id") {
        Ok(ids) => ids,
        Err(response) => return *response,
    };

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
        Err(e) => util::repo_error(&auth, e),
    }
}
