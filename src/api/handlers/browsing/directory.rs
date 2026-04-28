//! Directory browsing handlers.

use axum::response::IntoResponse;

use crate::api::auth::SubsonicContext;
use crate::api::error::ApiError;
use crate::api::handlers::browsing::IdParams;
use crate::api::response::{SubsonicResponse, error_response};
use crate::models::music::{ChildResponse, DirectoryResponse};

/// GET/POST /rest/getMusicDirectory[.view]
///
/// Returns a listing of all files in a music directory. Typically used to get
/// list of albums for an artist, or list of songs for an album.
/// The ID can refer to a music folder, artist, or album.
pub async fn get_music_directory(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<IdParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    // Get the required 'id' parameter
    let Some(id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    let maybe_album = match auth.music().get_album(id) {
        Ok(album) => album,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };
    let maybe_artist = match auth.music().get_artist(id) {
        Ok(artist) => artist,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let folders = match auth.music().get_music_folders() {
        Ok(folders) => folders,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };
    let maybe_folder = folders.iter().find(|f| f.id == id);
    let matches = usize::from(maybe_album.is_some())
        + usize::from(maybe_artist.is_some())
        + usize::from(maybe_folder.is_some());

    if matches > 1 {
        return error_response(
            auth.format,
            &ApiError::Generic(format!("Ambiguous directory id: {id}")),
        )
        .into_response();
    }

    if let Some(album) = maybe_album {
        let songs = match auth.music().get_songs_by_album(id) {
            Ok(songs) => songs,
            Err(e) => {
                return error_response(auth.format, &ApiError::Generic(e.to_string()))
                    .into_response();
            }
        };
        let children: Vec<ChildResponse> = songs.iter().map(ChildResponse::from).collect();
        let response = DirectoryResponse::from_album(&album, children);
        return SubsonicResponse::directory(auth.format, response).into_response();
    }

    if let Some(artist) = maybe_artist {
        let albums = match auth.music().get_albums_by_artist(id) {
            Ok(albums) => albums,
            Err(e) => {
                return error_response(auth.format, &ApiError::Generic(e.to_string()))
                    .into_response();
            }
        };
        let children: Vec<ChildResponse> = albums
            .iter()
            .map(ChildResponse::from_album_as_dir)
            .collect();
        let response = DirectoryResponse::from_artist(&artist, children);
        return SubsonicResponse::directory(auth.format, response).into_response();
    }

    if let Some(folder) = maybe_folder {
        let artists = match auth.music().get_artists_by_music_folder(folder.id) {
            Ok(artists) => artists,
            Err(e) => {
                return error_response(auth.format, &ApiError::Generic(e.to_string()))
                    .into_response();
            }
        };
        let children: Vec<ChildResponse> = artists
            .iter()
            .map(ChildResponse::from_artist_as_dir)
            .collect();
        let response = DirectoryResponse::from_music_folder(folder, children);
        return SubsonicResponse::directory(auth.format, response).into_response();
    }

    error_response(auth.format, &ApiError::NotFound("Directory".into())).into_response()
}
