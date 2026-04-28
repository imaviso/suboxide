//! Directory browsing handlers.

use axum::response::IntoResponse;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;
use crate::api::handlers::browsing::IdParams;
use crate::api::handlers::repo_result_or_response;
use crate::api::response::{SubsonicResponse, error_response};
use crate::models::music::{ChildResponse, DirectoryResponse};

/// GET/POST /rest/getMusicDirectory[.view]
///
/// Returns a listing of all files in a music directory. Typically used to get
/// list of albums for an artist, or list of songs for an album.
/// The ID can refer to a music folder, artist, or album.
pub async fn get_music_directory(
    axum::extract::Query(params): axum::extract::Query<IdParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Get the required 'id' parameter
    let Some(id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Try to find what this ID refers to: music folder, artist, or album
    // First, check if it's an album (most common case when browsing)
    let maybe_album = match repo_result_or_response(auth.format, auth.music().get_album(id)) {
        Ok(album) => album,
        Err(response) => return response,
    };
    if let Some(album) = maybe_album {
        let songs = match repo_result_or_response(auth.format, auth.music().get_songs_by_album(id))
        {
            Ok(songs) => songs,
            Err(response) => return response,
        };
        let children: Vec<ChildResponse> = songs.iter().map(ChildResponse::from).collect();
        let response = DirectoryResponse::from_album(&album, children);
        return SubsonicResponse::directory(auth.format, response).into_response();
    }

    // Check if it's an artist
    let maybe_artist = match repo_result_or_response(auth.format, auth.music().get_artist(id)) {
        Ok(artist) => artist,
        Err(response) => return response,
    };
    if let Some(artist) = maybe_artist {
        let albums =
            match repo_result_or_response(auth.format, auth.music().get_albums_by_artist(id)) {
                Ok(albums) => albums,
                Err(response) => return response,
            };
        let children: Vec<ChildResponse> = albums
            .iter()
            .map(ChildResponse::from_album_as_dir)
            .collect();
        let response = DirectoryResponse::from_artist(&artist, children);
        return SubsonicResponse::directory(auth.format, response).into_response();
    }

    // Check if it's a music folder
    let folders = match repo_result_or_response(auth.format, auth.music().get_music_folders()) {
        Ok(folders) => folders,
        Err(response) => return response,
    };
    if let Some(folder) = folders.iter().find(|f| f.id == id) {
        // For music folders, return all artists as children
        let artists = match repo_result_or_response(auth.format, auth.music().get_artists()) {
            Ok(artists) => artists,
            Err(response) => return response,
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
