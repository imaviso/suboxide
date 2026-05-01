//! Playlist-related API handlers (getPlaylists, getPlaylist, createPlaylist, updatePlaylist, deletePlaylist)
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::api::auth::SubsonicContext;
use crate::api::handlers::util;
use crate::api::response::SubsonicResponse;
use crate::db::Playlist;
use crate::models::music::{
    ChildResponse, PlaylistResponse, PlaylistWithSongsResponse, PlaylistsResponse, Song,
    format_subsonic_datetime,
};

fn playlist_with_songs_response(
    auth: &SubsonicContext,
    playlist: &Playlist,
    songs: &[Song],
) -> Result<PlaylistWithSongsResponse, Box<Response>> {
    let song_ids: Vec<i32> = songs.iter().map(|song| song.id).collect();
    let starred_map = auth
        .music()
        .get_starred_at_for_songs_batch(auth.user.id, &song_ids)
        .map_err(|error| Box::new(util::service_error(auth, error)))?;

    let entries = songs
        .iter()
        .map(|song| {
            let starred_at = starred_map.get(&song.id);
            ChildResponse::from_song_with_starred(song, starred_at)
        })
        .collect();
    let cover_art = songs.first().and_then(|song| song.cover_art.clone());

    Ok(PlaylistWithSongsResponse {
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
        entries,
    })
}

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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<GetPlaylistsParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let (user_id, username) = if let Some(username) = params.username.as_deref() {
        if username != auth.user.username && !auth.user.is_admin() {
            return util::unauthorized(&auth);
        }

        let user = match auth.users().find_user(username) {
            Ok(Some(user)) => user,
            Ok(None) => {
                return util::not_found(&auth, "User");
            }
            Err(e) => {
                return util::service_error(&auth, e);
            }
        };
        (user.id, user.username)
    } else {
        (auth.user.id, auth.user.username.clone())
    };

    let playlists = match auth.music().get_playlists(user_id, &username) {
        Ok(v) => v,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    let playlist_ids: Vec<i32> = playlists.iter().map(|p| p.id).collect();
    let cover_arts = match auth.music().get_playlist_cover_arts_batch(&playlist_ids) {
        Ok(v) => v,
        Err(e) => {
            return util::service_error(&auth, e);
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<GetPlaylistParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(id_str) = params.id.as_ref() else {
        return util::missing_param(&auth, "id");
    };
    let Ok(playlist_id) = id_str.parse::<i32>() else {
        return util::service_error(&auth, format!("Invalid id: {id_str}"));
    };

    let playlist = match auth.music().get_playlist(playlist_id) {
        Ok(Some(p)) => p,
        Ok(None) => {
            return util::not_found(&auth, "Playlist");
        }
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    if playlist.owner != auth.user.username && !playlist.public {
        return util::unauthorized(&auth);
    }

    let songs = match auth.music().get_playlist_songs(playlist_id) {
        Ok(v) => v,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };
    let response = match playlist_with_songs_response(&auth, &playlist, &songs) {
        Ok(response) => response,
        Err(response) => return *response,
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
pub async fn create_playlist(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<CreatePlaylistParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    if !auth.user.roles.playlist_role {
        return util::unauthorized(&auth);
    }

    let user_id = auth.user.id;
    let song_ids = params.song_id;

    if let Some(playlist_id_str) = params.playlist_id.as_ref() {
        let Ok(playlist_id) = playlist_id_str.parse::<i32>() else {
            return util::service_error(&auth, "Invalid playlistId");
        };

        match auth.music().is_playlist_owner(user_id, playlist_id) {
            Ok(true) => {}
            Ok(false) => {
                return util::unauthorized(&auth);
            }
            Err(e) => {
                return util::service_error(&auth, e);
            }
        }

        if let Err(e) = auth.music().update_playlist(
            playlist_id,
            params.name.as_deref(),
            None,
            None,
            &song_ids,
            &[],
        ) {
            return util::service_error(&auth, e);
        }

        let playlist = match auth.music().get_playlist(playlist_id) {
            Ok(Some(p)) => p,
            Ok(None) => {
                return util::not_found(&auth, "Playlist");
            }
            Err(e) => {
                return util::service_error(&auth, e);
            }
        };
        let songs = match auth.music().get_playlist_songs(playlist_id) {
            Ok(v) => v,
            Err(e) => {
                return util::service_error(&auth, e);
            }
        };

        let response = match playlist_with_songs_response(&auth, &playlist, &songs) {
            Ok(response) => response,
            Err(response) => return *response,
        };

        return SubsonicResponse::playlist(auth.format, response).into_response();
    }

    let name = match params.name.as_deref() {
        Some(n) if !n.is_empty() => n,
        _ => {
            return util::missing_param(&auth, "name");
        }
    };

    let playlist = match auth.music().create_playlist(user_id, name, None, &song_ids) {
        Ok(p) => p,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    let songs = match auth.music().get_playlist_songs(playlist.id) {
        Ok(v) => v,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    let response = match playlist_with_songs_response(&auth, &playlist, &songs) {
        Ok(response) => response,
        Err(response) => return *response,
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<UpdatePlaylistParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    if !auth.user.roles.playlist_role {
        return util::unauthorized(&auth);
    }

    let user_id = auth.user.id;

    let Some(id_str) = params.playlist_id.as_ref() else {
        return util::missing_param(&auth, "playlistId");
    };
    let Ok(playlist_id) = id_str.parse::<i32>() else {
        return util::service_error(&auth, format!("Invalid playlistId: {id_str}"));
    };

    match auth.music().is_playlist_owner(user_id, playlist_id) {
        Ok(true) => {}
        Ok(false) => return util::unauthorized(&auth),
        Err(e) => {
            return util::service_error(&auth, e);
        }
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
        Err(e) => util::service_error(&auth, e),
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<DeletePlaylistParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    if !auth.user.roles.playlist_role {
        return util::unauthorized(&auth);
    }

    let Some(id_str) = params.id.as_ref() else {
        return util::missing_param(&auth, "id");
    };
    let Ok(playlist_id) = id_str.parse::<i32>() else {
        return util::service_error(&auth, format!("Invalid id: {id_str}"));
    };

    let user_id = auth.user.id;

    match auth.music().is_playlist_owner(user_id, playlist_id) {
        Ok(true) => {}
        Ok(false) => return util::unauthorized(&auth),
        Err(e) => {
            return util::service_error(&auth, e);
        }
    }

    match auth.music().delete_playlist(playlist_id) {
        Ok(true) => SubsonicResponse::empty(auth.format).into_response(),
        Ok(false) => util::not_found(&auth, "Playlist"),
        Err(e) => util::service_error(&auth, e),
    }
}
