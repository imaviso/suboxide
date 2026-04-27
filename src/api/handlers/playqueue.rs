//! Play queue API handlers (getPlayQueue, savePlayQueue, getPlayQueueByIndex, savePlayQueueByIndex)
use axum::extract::RawQuery;
use axum::response::IntoResponse;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;
use crate::api::handlers::repo_result_or_response;
use crate::api::response::{SubsonicResponse, error_response};
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

    match repo_result_or_response(auth.format, auth.state().get_play_queue(user_id, username)) {
        Ok(Some(play_queue)) => {
            let song_ids: Vec<i32> = play_queue.songs.iter().map(|s| s.id).collect();
            let starred_map = match repo_result_or_response(
                auth.format,
                auth.state()
                    .get_starred_at_for_songs_batch(user_id, &song_ids),
            ) {
                Ok(v) => v,
                Err(response) => return response,
            };

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
        Err(response) => response,
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

    let song_id_strs = parse_repeated_param(&query, "id");
    let song_ids = match crate::api::handlers::parse_i32_list(auth.format, &song_id_strs, "id") {
        Ok(ids) => ids,
        Err(response) => return response,
    };

    let current_song_id = match parse_repeated_param(&query, "current").as_slice() {
        [] => None,
        [id] => match id.parse::<i32>() {
            Ok(v) => Some(v),
            Err(_) => {
                return error_response(
                    auth.format,
                    &ApiError::Generic(format!("Invalid current: {id}")),
                )
                .into_response();
            }
        },
        _ => {
            return error_response(
                auth.format,
                &ApiError::Generic("Multiple current values provided".into()),
            )
            .into_response();
        }
    };

    let position = match parse_repeated_param(&query, "position").as_slice() {
        [] => None,
        [p] => match p.parse::<i64>() {
            Ok(v) => Some(v),
            Err(_) => {
                return error_response(
                    auth.format,
                    &ApiError::Generic(format!("Invalid position: {p}")),
                )
                .into_response();
            }
        },
        _ => {
            return error_response(
                auth.format,
                &ApiError::Generic("Multiple position values provided".into()),
            )
            .into_response();
        }
    };

    let changed_by = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    match repo_result_or_response(
        auth.format,
        auth.state()
            .save_play_queue(user_id, &song_ids, current_song_id, position, changed_by),
    ) {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(response) => response,
    }
}

/// GET/POST /rest/getPlayQueueByIndex[.view]
///
/// Returns the current play queue for the user using queue index instead of song ID.
/// This is an `OpenSubsonic` extension.
pub async fn get_play_queue_by_index(auth: SubsonicAuth) -> impl IntoResponse {
    let user_id = auth.user.id;
    let username = &auth.user.username;

    match repo_result_or_response(auth.format, auth.state().get_play_queue(user_id, username)) {
        Ok(Some(play_queue)) => {
            let song_ids: Vec<i32> = play_queue.songs.iter().map(|s| s.id).collect();
            let starred_map = match repo_result_or_response(
                auth.format,
                auth.state()
                    .get_starred_at_for_songs_batch(user_id, &song_ids),
            ) {
                Ok(v) => v,
                Err(response) => return response,
            };

            let song_responses: Vec<ChildResponse> = play_queue
                .songs
                .iter()
                .map(|s| {
                    let starred_at = starred_map.get(&s.id);
                    ChildResponse::from_song_with_starred(s, starred_at)
                })
                .collect();

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
        Err(response) => response,
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

    let song_id_strs = parse_repeated_param(&query, "id");
    let song_ids = match crate::api::handlers::parse_i32_list(auth.format, &song_id_strs, "id") {
        Ok(ids) => ids,
        Err(response) => return response,
    };

    let current_index = match parse_repeated_param(&query, "currentIndex").as_slice() {
        [] => None,
        [idx] => match idx.parse::<usize>() {
            Ok(v) => Some(v),
            Err(_) => {
                return error_response(
                    auth.format,
                    &ApiError::Generic(format!("Invalid currentIndex: {idx}")),
                )
                .into_response();
            }
        },
        _ => {
            return error_response(
                auth.format,
                &ApiError::Generic("Multiple currentIndex values provided".into()),
            )
            .into_response();
        }
    };

    let current_song_id = current_index.and_then(|idx| song_ids.get(idx).copied());

    let position = match parse_repeated_param(&query, "position").as_slice() {
        [] => None,
        [p] => match p.parse::<i64>() {
            Ok(v) => Some(v),
            Err(_) => {
                return error_response(
                    auth.format,
                    &ApiError::Generic(format!("Invalid position: {p}")),
                )
                .into_response();
            }
        },
        _ => {
            return error_response(
                auth.format,
                &ApiError::Generic("Multiple position values provided".into()),
            )
            .into_response();
        }
    };

    let changed_by = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    match repo_result_or_response(
        auth.format,
        auth.state()
            .save_play_queue(user_id, &song_ids, current_song_id, position, changed_by),
    ) {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(response) => response,
    }
}
