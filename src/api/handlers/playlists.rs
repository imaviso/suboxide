//! Playlist-related API handlers (getPlaylists, getPlaylist, createPlaylist, updatePlaylist, deletePlaylist)
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;
use crate::api::response::{SubsonicResponse, error_response};
use crate::models::music::{
    ChildResponse, PlaylistResponse, PlaylistWithSongsResponse, PlaylistsResponse,
    format_subsonic_datetime,
};

/// Query parameters for getPlaylists.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GetPlaylistsParams {
    /// If specified, return playlists for this user rather than the authenticated user.
    /// Only admins can view other users' playlists.
    pub username: Option<String>,
}

/// GET/POST /rest/getPlaylists[.view]
///
/// Returns all playlists a user is allowed to play.
pub async fn get_playlists(
    axum::extract::Query(_params): axum::extract::Query<GetPlaylistsParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let user_id = auth.user.id;
    let username = &auth.user.username;

    let playlists = match auth.music().get_playlists(user_id, username) {
        Ok(v) => v,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let playlist_ids: Vec<i32> = playlists.iter().map(|p| p.id).collect();
    let cover_arts = match auth.music().get_playlist_cover_arts_batch(&playlist_ids) {
        Ok(v) => v,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let playlist_responses: Vec<PlaylistResponse> = playlists
        .iter()
        .map(|p| {
            let cover_art = cover_arts.get(&p.id).cloned();
            PlaylistResponse {
                id: p.id.to_string(),
                name: p.name.clone(),
                comment: p.comment.clone(),
                owner: p.owner.clone(),
                public: p.public,
                song_count: p.song_count,
                duration: p.duration,
                created: format_subsonic_datetime(&p.created_at),
                changed: format_subsonic_datetime(&p.updated_at),
                cover_art,
            }
        })
        .collect();

    let response = PlaylistsResponse {
        playlists: playlist_responses,
    };

    SubsonicResponse::playlists(auth.format, response).into_response()
}

/// Query parameters for getPlaylist.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GetPlaylistParams {
    /// The ID of the playlist to retrieve.
    pub id: Option<String>,
}

/// GET/POST /rest/getPlaylist[.view]
///
/// Returns a listing of files in a saved playlist.
pub async fn get_playlist(
    axum::extract::Query(params): axum::extract::Query<GetPlaylistParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(id_str) = params.id.as_ref() else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };
    let Ok(playlist_id) = id_str.parse::<i32>() else {
        return error_response(
            auth.format,
            &ApiError::Generic(format!("Invalid id: {id_str}")),
        )
        .into_response();
    };

    let playlist = match auth.music().get_playlist(playlist_id) {
        Ok(Some(p)) => p,
        Ok(None) => {
            return error_response(auth.format, &ApiError::NotFound("Playlist".into()))
                .into_response();
        }
        Err(e) => return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response(),
    };

    if playlist.owner != auth.user.username && !playlist.public {
        return error_response(auth.format, &ApiError::NotAuthorized).into_response();
    }

    let songs = match auth.music().get_playlist_songs(playlist_id) {
        Ok(v) => v,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };
    let user_id = auth.user.id;

    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();
    let starred_map = match auth
        .music()
        .get_starred_at_for_songs_batch(user_id, &song_ids)
    {
        Ok(v) => v,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|s| {
            let starred_at = starred_map.get(&s.id);
            ChildResponse::from_song_with_starred(s, starred_at)
        })
        .collect();

    let cover_art = songs.first().and_then(|s| s.cover_art.clone());

    let response = PlaylistWithSongsResponse {
        id: playlist.id.to_string(),
        name: playlist.name.clone(),
        comment: playlist.comment.clone(),
        owner: playlist.owner.clone(),
        public: playlist.public,
        song_count: playlist.song_count,
        duration: playlist.duration,
        created: format_subsonic_datetime(&playlist.created_at),
        changed: format_subsonic_datetime(&playlist.updated_at),
        cover_art,
        entries: song_responses,
    };

    SubsonicResponse::playlist(auth.format, response).into_response()
}

/// Query parameters for createPlaylist.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CreatePlaylistParams {
    /// The playlist ID to update (if updating an existing playlist).
    #[serde(rename = "playlistId")]
    pub playlist_id: Option<String>,
    /// The playlist name (required if creating a new playlist).
    pub name: Option<String>,
    /// IDs of songs to add (can be repeated).
    #[serde(rename = "songId")]
    pub song_id: Vec<i32>,
}

/// GET/POST /rest/createPlaylist[.view]
///
/// Creates (or updates) a playlist.
///
/// Parameters:
/// - `playlistId`: The playlist ID (if updating an existing playlist)
/// - `name`: The playlist name (required if creating a new playlist)
/// - `songId`: ID of a song to add (can be repeated)
#[expect(
    clippy::too_many_lines,
    reason = "Playlist creation supports create/update flows and repeated query parameters"
)]
pub async fn create_playlist(
    axum::extract::Query(params): axum::extract::Query<CreatePlaylistParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let user_id = auth.user.id;
    let song_ids = params.song_id;

    if let Some(playlist_id_str) = params.playlist_id.as_ref() {
        let Ok(playlist_id) = playlist_id_str.parse::<i32>() else {
            return error_response(auth.format, &ApiError::Generic("Invalid playlistId".into()))
                .into_response();
        };

        match auth.music().is_playlist_owner(user_id, playlist_id) {
            Ok(true) => {}
            Ok(false) => {
                return error_response(auth.format, &ApiError::NotAuthorized).into_response();
            }
            Err(e) => return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response(),
        }

        if let Err(e) = auth.music().update_playlist(
            playlist_id,
            params.name.as_deref(),
            None,
            None,
            &song_ids,
            &[],
        ) {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }

        let playlist = match auth.music().get_playlist(playlist_id) {
            Ok(Some(p)) => p,
            Ok(None) => {
                return error_response(auth.format, &ApiError::NotFound("Playlist".into()))
                    .into_response();
            }
            Err(e) => return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response(),
        };
        let songs = match auth.music().get_playlist_songs(playlist_id) {
            Ok(v) => v,
            Err(e) => {
                return error_response(auth.format, &ApiError::Generic(e.to_string()))
                    .into_response();
            }
        };

        let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();
        let starred_map = match auth
            .music()
            .get_starred_at_for_songs_batch(user_id, &song_ids)
        {
            Ok(v) => v,
            Err(e) => {
                return error_response(auth.format, &ApiError::Generic(e.to_string()))
                    .into_response();
            }
        };

        let song_responses: Vec<ChildResponse> = songs
            .iter()
            .map(|s| {
                let starred_at = starred_map.get(&s.id);
                ChildResponse::from_song_with_starred(s, starred_at)
            })
            .collect();

        let cover_art = songs.first().and_then(|s| s.cover_art.clone());

        let response = PlaylistWithSongsResponse {
            id: playlist.id.to_string(),
            name: playlist.name.clone(),
            comment: playlist.comment.clone(),
            owner: playlist.owner.clone(),
            public: playlist.public,
            song_count: playlist.song_count,
            duration: playlist.duration,
            created: format_subsonic_datetime(&playlist.created_at),
            changed: format_subsonic_datetime(&playlist.updated_at),
            cover_art,
            entries: song_responses,
        };

        return SubsonicResponse::playlist(auth.format, response).into_response();
    }

    let name = match params.name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return error_response(auth.format, &ApiError::MissingParameter("name".into()))
                .into_response();
        }
    };

    let playlist = match auth.music().create_playlist(user_id, name, None, &song_ids) {
        Ok(p) => p,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let songs = match auth.music().get_playlist_songs(playlist.id) {
        Ok(v) => v,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();
    let starred_map = match auth
        .music()
        .get_starred_at_for_songs_batch(user_id, &song_ids)
    {
        Ok(v) => v,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|s| {
            let starred_at = starred_map.get(&s.id);
            ChildResponse::from_song_with_starred(s, starred_at)
        })
        .collect();

    let cover_art = songs.first().and_then(|s| s.cover_art.clone());

    let response = PlaylistWithSongsResponse {
        id: playlist.id.to_string(),
        name: playlist.name.clone(),
        comment: playlist.comment.clone(),
        owner: playlist.owner.clone(),
        public: playlist.public,
        song_count: playlist.song_count,
        duration: playlist.duration,
        created: format_subsonic_datetime(&playlist.created_at),
        changed: format_subsonic_datetime(&playlist.updated_at),
        cover_art,
        entries: song_responses,
    };

    SubsonicResponse::playlist(auth.format, response).into_response()
}

/// Query parameters for updatePlaylist.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UpdatePlaylistParams {
    /// The playlist ID.
    #[serde(rename = "playlistId")]
    pub playlist_id: Option<String>,
    /// The new name.
    pub name: Option<String>,
    /// The new comment.
    pub comment: Option<String>,
    /// Whether the playlist is public.
    pub public: Option<bool>,
    /// Song IDs to add (can be repeated).
    #[serde(rename = "songIdToAdd")]
    pub song_id_to_add: Vec<i32>,
    /// Indices (0-based) of songs to remove (can be repeated).
    #[serde(rename = "songIndexToRemove")]
    pub song_index_to_remove: Vec<i32>,
}

/// GET/POST /rest/updatePlaylist[.view]
///
/// Updates a playlist. Only the owner can update a playlist.
///
/// Parameters:
/// - `playlistId`: The playlist ID (required)
/// - `name`: The new name
/// - `comment`: The new comment
/// - `public`: Whether the playlist is public
/// - `songIdToAdd`: Song ID to add (can be repeated)
/// - `songIndexToRemove`: Index (0-based) of song to remove (can be repeated)
pub async fn update_playlist(
    axum::extract::Query(params): axum::extract::Query<UpdatePlaylistParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let user_id = auth.user.id;

    let Some(id_str) = params.playlist_id.as_ref() else {
        return error_response(
            auth.format,
            &ApiError::MissingParameter("playlistId".into()),
        )
        .into_response();
    };
    let Ok(playlist_id) = id_str.parse::<i32>() else {
        return error_response(
            auth.format,
            &ApiError::Generic(format!("Invalid playlistId: {id_str}")),
        )
        .into_response();
    };

    match auth.music().is_playlist_owner(user_id, playlist_id) {
        Ok(true) => {}
        Ok(false) => return error_response(auth.format, &ApiError::NotAuthorized).into_response(),
        Err(e) => return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response(),
    }

    match auth.music().update_playlist(
        playlist_id,
        params.name.as_deref(),
        params.comment.as_deref(),
        params.public,
        &params.song_id_to_add,
        &params.song_index_to_remove,
    ) {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(e) => error_response(auth.format, &ApiError::Generic(e.to_string())).into_response(),
    }
}

/// Query parameters for deletePlaylist.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DeletePlaylistParams {
    /// The ID of the playlist to delete.
    pub id: Option<String>,
}

/// GET/POST /rest/deletePlaylist[.view]
///
/// Deletes a playlist.
pub async fn delete_playlist(
    axum::extract::Query(params): axum::extract::Query<DeletePlaylistParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(id_str) = params.id.as_ref() else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };
    let Ok(playlist_id) = id_str.parse::<i32>() else {
        return error_response(
            auth.format,
            &ApiError::Generic(format!("Invalid id: {id_str}")),
        )
        .into_response();
    };

    let user_id = auth.user.id;

    match auth.music().is_playlist_owner(user_id, playlist_id) {
        Ok(true) => {}
        Ok(false) => return error_response(auth.format, &ApiError::NotAuthorized).into_response(),
        Err(e) => return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response(),
    }

    match auth.music().delete_playlist(playlist_id) {
        Ok(true) => SubsonicResponse::empty(auth.format).into_response(),
        Ok(false) => {
            error_response(auth.format, &ApiError::NotFound("Playlist".into())).into_response()
        }
        Err(e) => error_response(auth.format, &ApiError::Generic(e.to_string())).into_response(),
    }
}
