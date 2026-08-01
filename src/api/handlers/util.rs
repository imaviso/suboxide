//! Shared handler response helpers.

use axum::response::{IntoResponse, Response};

use crate::api::auth::SubsonicContext;
use crate::api::error::ApiError;
use crate::api::response::error_response;
use crate::models::music::{Album, AlbumID3Response, ChildResponse, Song};

/// Build a missing-parameter response for the current request format.
pub(in crate::api::handlers) fn missing_param(auth: &SubsonicContext, name: &str) -> Response {
    api_error(auth, &ApiError::MissingParameter(name.into()))
}

/// Build a not-found response for the current request format.
pub(in crate::api::handlers) fn not_found(auth: &SubsonicContext, resource: &str) -> Response {
    api_error(auth, &ApiError::NotFound(resource.into()))
}

/// Build a not-authorized response for the current request format.
pub(in crate::api::handlers) fn unauthorized(auth: &SubsonicContext) -> Response {
    api_error(auth, &ApiError::NotAuthorized)
}

/// Build a generic service error response for the current request format.
pub(in crate::api::handlers) fn service_error(
    auth: &SubsonicContext,
    error: impl std::fmt::Display,
) -> Response {
    api_error(auth, &ApiError::Generic(error.to_string()))
}

/// Build a formatted API error response.
pub(in crate::api::handlers) fn api_error(auth: &SubsonicContext, error: &ApiError) -> Response {
    error_response(auth.format, error).into_response()
}

/// Build song responses with starred, rating, last-played, and bookmark data
/// attached for the current user.
pub(in crate::api::handlers) fn annotate_songs(
    auth: &SubsonicContext,
    songs: &[Song],
) -> Result<Vec<ChildResponse>, crate::db::MusicRepoError> {
    let song_ids: Vec<i32> = songs.iter().map(|song| song.id).collect();
    let starred = auth
        .music()
        .get_starred_at_for_songs_batch(auth.user.id, &song_ids)?;
    let annotations = auth
        .music()
        .get_song_annotations_batch(auth.user.id, &song_ids)?;

    Ok(songs
        .iter()
        .map(|song| {
            let mut entry = annotations.get(&song.id).copied().unwrap_or_default();
            entry.starred_at = starred.get(&song.id).copied();
            ChildResponse::from(song).with_annotations(Some(&entry))
        })
        .collect())
}

/// Build album responses with starred, rating, and last-played data attached
/// for the current user.
pub(in crate::api::handlers) fn annotate_albums(
    auth: &SubsonicContext,
    albums: &[Album],
) -> Result<Vec<AlbumID3Response>, crate::db::MusicRepoError> {
    let album_ids: Vec<i32> = albums.iter().map(|album| album.id).collect();
    let starred = auth
        .music()
        .get_starred_at_for_albums_batch(auth.user.id, &album_ids)?;
    let annotations = auth
        .music()
        .get_album_annotations_batch(auth.user.id, &album_ids)?;

    Ok(albums
        .iter()
        .map(|album| {
            AlbumID3Response::from_album_with_starred(album, starred.get(&album.id))
                .with_annotations(annotations.get(&album.id))
        })
        .collect())
}
