//! Play queue API handlers (getPlayQueue, savePlayQueue, getPlayQueueByIndex, savePlayQueueByIndex)
use axum::extract::RawQuery;
use axum::response::IntoResponse;

use crate::api::auth::SubsonicAuth;
use crate::api::response::SubsonicResponse;
use crate::models::music::{
    ChildResponse, PlayQueueByIndexResponse, PlayQueueResponse, format_subsonic_datetime,
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

/// GET/POST /rest/getPlayQueue[.view]
///
/// Returns the current play queue for the user.
pub async fn get_play_queue(auth: SubsonicAuth) -> impl IntoResponse {
    let user_id = auth.user.id;
    let username = &auth.user.username;

    if let Some(play_queue) = auth.state.get_play_queue(user_id, username) {
        // Batch fetch starred status for all songs
        let song_ids: Vec<i32> = play_queue.songs.iter().map(|s| s.id).collect();
        let starred_map = auth
            .state
            .get_starred_at_for_songs_batch(user_id, &song_ids);

        let song_responses: Vec<ChildResponse> = play_queue
            .songs
            .iter()
            .map(|s| {
                let starred_at = starred_map.get(&s.id);
                ChildResponse::from_song_with_starred(s, starred_at)
            })
            .collect();

        let response = PlayQueueResponse {
            current: play_queue.current_song.as_ref().map(|s| s.id.to_string()),
            position: play_queue.position,
            username: play_queue.username.clone(),
            changed: format_subsonic_datetime(&play_queue.changed_at),
            changed_by: play_queue.changed_by.clone(),
            entries: song_responses,
        };

        SubsonicResponse::play_queue(auth.format, response)
    } else {
        // Return empty play queue
        let response = PlayQueueResponse {
            current: None,
            position: None,
            username: username.clone(),
            changed: format_subsonic_datetime(&chrono::Utc::now().naive_utc()),
            changed_by: None,
            entries: vec![],
        };

        SubsonicResponse::play_queue(auth.format, response)
    }
}

/// GET/POST /rest/savePlayQueue[.view]
///
/// Saves the current play queue for the user.
///
/// Parameters:
/// - `id`: ID of a song in the play queue (can be repeated to define the entire queue)
/// - `current`: The ID of the currently playing song
/// - `position`: Position in milliseconds within the currently playing song
pub async fn save_play_queue(RawQuery(query): RawQuery, auth: SubsonicAuth) -> impl IntoResponse {
    let query = query.unwrap_or_default();
    let user_id = auth.user.id;

    // Parse song IDs from repeated parameters
    let song_ids: Vec<i32> = parse_repeated_param(&query, "id")
        .iter()
        .filter_map(|id| id.parse::<i32>().ok())
        .collect();

    // Parse current song ID
    let current_song_id: Option<i32> = parse_repeated_param(&query, "current")
        .first()
        .and_then(|id| id.parse::<i32>().ok());

    // Parse position
    let position: Option<i64> = parse_repeated_param(&query, "position")
        .first()
        .and_then(|p| p.parse::<i64>().ok());

    // Get the client identifier as changed_by
    let changed_by = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    if let Err(e) =
        auth.state
            .save_play_queue(user_id, &song_ids, current_song_id, position, changed_by)
    {
        tracing::event!(
            name: "playqueue.save.failed",
            tracing::Level::WARN,
            error = %e,
            "play queue save failed"
        );
        // Don't return an error - the API spec says this should succeed silently
    }

    SubsonicResponse::empty(auth.format)
}

/// GET/POST /rest/getPlayQueueByIndex[.view]
///
/// Returns the current play queue for the user using queue index instead of song ID.
/// This is an `OpenSubsonic` extension.
pub async fn get_play_queue_by_index(auth: SubsonicAuth) -> impl IntoResponse {
    let user_id = auth.user.id;
    let username = &auth.user.username;

    if let Some(play_queue) = auth.state.get_play_queue(user_id, username) {
        // Batch fetch starred status for all songs
        let song_ids: Vec<i32> = play_queue.songs.iter().map(|s| s.id).collect();
        let starred_map = auth
            .state
            .get_starred_at_for_songs_batch(user_id, &song_ids);

        let song_responses: Vec<ChildResponse> = play_queue
            .songs
            .iter()
            .map(|s| {
                let starred_at = starred_map.get(&s.id);
                ChildResponse::from_song_with_starred(s, starred_at)
            })
            .collect();

        // Calculate current_index by finding the position of the current song in the queue
        let current_index = play_queue.current_song.as_ref().and_then(|current_song| {
            play_queue
                .songs
                .iter()
                .position(|s| s.id == current_song.id)
                .and_then(|idx| i32::try_from(idx).ok())
        });

        let response = PlayQueueByIndexResponse {
            current_index,
            position: play_queue.position,
            username: play_queue.username.clone(),
            changed: format_subsonic_datetime(&play_queue.changed_at),
            changed_by: play_queue.changed_by.clone(),
            entries: song_responses,
        };

        SubsonicResponse::play_queue_by_index(auth.format, response)
    } else {
        // Return empty play queue
        let response = PlayQueueByIndexResponse {
            current_index: None,
            position: None,
            username: username.clone(),
            changed: format_subsonic_datetime(&chrono::Utc::now().naive_utc()),
            changed_by: None,
            entries: vec![],
        };

        SubsonicResponse::play_queue_by_index(auth.format, response)
    }
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
    RawQuery(query): RawQuery,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let query = query.unwrap_or_default();
    let user_id = auth.user.id;

    // Parse song IDs from repeated parameters
    let song_ids: Vec<i32> = parse_repeated_param(&query, "id")
        .iter()
        .filter_map(|id| id.parse::<i32>().ok())
        .collect();

    // Parse current index (0-based index into the song_ids array)
    let current_index: Option<usize> = parse_repeated_param(&query, "currentIndex")
        .first()
        .and_then(|idx| idx.parse::<usize>().ok());

    // Convert current_index to current_song_id
    let current_song_id = current_index.and_then(|idx| song_ids.get(idx).copied());

    // Parse position
    let position: Option<i64> = parse_repeated_param(&query, "position")
        .first()
        .and_then(|p| p.parse::<i64>().ok());

    // Get the client identifier as changed_by
    let changed_by = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    if let Err(e) =
        auth.state
            .save_play_queue(user_id, &song_ids, current_song_id, position, changed_by)
    {
        tracing::event!(
            name: "playqueue.save_by_index.failed",
            tracing::Level::WARN,
            error = %e,
            "play queue save by index failed"
        );
        // Don't return an error - the API spec says this should succeed silently
    }

    SubsonicResponse::empty(auth.format)
}
