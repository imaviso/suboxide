//! Bookmark handlers (getBookmarks, createBookmark, deleteBookmark).
//!
//! A bookmark is a position within a media file, used to resume playback.
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicContext;
use crate::api::handlers::util;
use crate::api::response::SubsonicResponse;
use crate::models::music::{
    BookmarkResponse, BookmarksResponse, ChildResponse, format_subsonic_datetime,
};

/// GET/POST /rest/getBookmarks[.view]
///
/// Returns all bookmarks for this user.
pub async fn get_bookmarks(auth: SubsonicContext) -> impl IntoResponse {
    let entries = match auth.music().get_bookmarks(auth.user.id) {
        Ok(entries) => entries,
        Err(error) => return util::service_error(&auth, error),
    };

    let song_ids: Vec<i32> = entries.iter().map(|entry| entry.song.id).collect();
    let annotations = match auth
        .music()
        .get_song_annotations_batch(auth.user.id, &song_ids)
    {
        Ok(annotations) => annotations,
        Err(error) => return util::service_error(&auth, error),
    };

    let bookmarks: Vec<BookmarkResponse> = entries
        .iter()
        .map(|entry| BookmarkResponse {
            position: entry.position,
            username: auth.user.username.clone(),
            comment: entry.comment.clone(),
            created: format_subsonic_datetime(&entry.created_at),
            changed: format_subsonic_datetime(&entry.updated_at),
            entry: ChildResponse::from(&entry.song)
                .with_annotations(annotations.get(&entry.song.id)),
        })
        .collect();

    SubsonicResponse::bookmarks(auth.format, BookmarksResponse { bookmarks }).into_response()
}

/// Query parameters for createBookmark.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CreateBookmarkParams {
    /// ID of the song to bookmark.
    pub id: Option<String>,
    /// Position in milliseconds within the song.
    pub position: Option<i64>,
    /// A user-defined comment.
    pub comment: Option<String>,
}

/// GET/POST /rest/createBookmark[.view]
///
/// Creates or updates a bookmark (a position within a media file).
/// Bookmarks are personal and not visible to other users.
pub async fn create_bookmark(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<CreateBookmarkParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(song_id) = params
        .id
        .as_deref()
        .and_then(crate::models::music::EntityId::parse_song)
    else {
        return util::missing_param(&auth, "id");
    };
    let Some(position) = params.position else {
        return util::missing_param(&auth, "position");
    };
    if position < 0 {
        return util::service_error(&auth, "Position must be non-negative");
    }

    match auth
        .music()
        .create_bookmark(auth.user.id, song_id, position, params.comment.as_deref())
    {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(error) => util::service_error(&auth, error),
    }
}

/// Query parameters for deleteBookmark.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DeleteBookmarkParams {
    /// ID of the song whose bookmark to delete.
    pub id: Option<String>,
}

/// GET/POST /rest/deleteBookmark[.view]
///
/// Deletes the bookmark for a given song.
pub async fn delete_bookmark(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<DeleteBookmarkParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(song_id) = params
        .id
        .as_deref()
        .and_then(crate::models::music::EntityId::parse_song)
    else {
        return util::missing_param(&auth, "id");
    };

    match auth.music().delete_bookmark(auth.user.id, song_id) {
        Ok(_) => SubsonicResponse::empty(auth.format).into_response(),
        Err(error) => util::service_error(&auth, error),
    }
}
