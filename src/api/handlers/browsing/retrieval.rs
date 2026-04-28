//! Entity retrieval handlers (album, artist, song).

use axum::response::IntoResponse;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;
use crate::api::handlers::browsing::IdParams;
use crate::api::response::{SubsonicResponse, error_response};
use crate::models::music::{
    AlbumID3Response, AlbumWithSongsID3Response, ArtistWithAlbumsID3Response, ChildResponse,
};

/// GET/POST /rest/getAlbum[.view]
///
/// Returns details for an album, including its songs.
pub async fn get_album(
    axum::extract::Query(params): axum::extract::Query<IdParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(album_id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    let album = match auth.music().get_album(album_id) {
        Ok(Some(album)) => album,
        Ok(None) => {
            return error_response(auth.format, &ApiError::NotFound("Album".into()))
                .into_response();
        }
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let album_starred_at = match auth
        .music()
        .get_starred_at_for_album(auth.user.id, album_id)
    {
        Ok(starred_at) => starred_at,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let songs = match auth.music().get_songs_by_album(album_id) {
        Ok(songs) => songs,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();
    let starred_songs = match auth
        .music()
        .get_starred_at_for_songs_batch(auth.user.id, &song_ids)
    {
        Ok(starred) => starred,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|song| {
            let starred_at = starred_songs.get(&song.id);
            ChildResponse::from_song_with_starred(song, starred_at)
        })
        .collect();

    let response = AlbumWithSongsID3Response::from_album_and_songs_with_starred(
        &album,
        song_responses,
        album_starred_at.as_ref(),
    );
    SubsonicResponse::album(auth.format, response).into_response()
}

/// GET/POST /rest/getArtist[.view]
///
/// Returns details for an artist, including their albums.
pub async fn get_artist(
    axum::extract::Query(params): axum::extract::Query<IdParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(artist_id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    let artist = match auth.music().get_artist(artist_id) {
        Ok(Some(artist)) => artist,
        Ok(None) => {
            return error_response(auth.format, &ApiError::NotFound("Artist".into()))
                .into_response();
        }
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let artist_starred_at = match auth
        .music()
        .get_starred_at_for_artist(auth.user.id, artist_id)
    {
        Ok(starred_at) => starred_at,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let albums = match auth.music().get_albums_by_artist(artist_id) {
        Ok(albums) => albums,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };
    let album_ids: Vec<i32> = albums.iter().map(|a| a.id).collect();
    let starred_map = match auth
        .music()
        .get_starred_at_for_albums_batch(auth.user.id, &album_ids)
    {
        Ok(starred) => starred,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let album_responses: Vec<AlbumID3Response> = albums
        .iter()
        .map(|album| {
            let starred_at = starred_map.get(&album.id);
            AlbumID3Response::from_album_with_starred(album, starred_at)
        })
        .collect();

    let response = ArtistWithAlbumsID3Response::from_artist_and_albums_with_starred(
        &artist,
        album_responses,
        artist_starred_at.as_ref(),
    );
    SubsonicResponse::artist(auth.format, response).into_response()
}

/// GET/POST /rest/getSong[.view]
///
/// Returns details for a song.
pub async fn get_song(
    axum::extract::Query(params): axum::extract::Query<IdParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(song_id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    let song = match auth.music().get_song(song_id) {
        Ok(Some(song)) => song,
        Ok(None) => {
            return error_response(auth.format, &ApiError::NotFound("Song".into())).into_response();
        }
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };

    let starred_at = match auth.music().get_starred_at_for_song(auth.user.id, song_id) {
        Ok(starred_at) => starred_at,
        Err(e) => {
            return error_response(auth.format, &ApiError::Generic(e.to_string())).into_response();
        }
    };
    let response = ChildResponse::from_song_with_starred(&song, starred_at.as_ref());
    SubsonicResponse::song(auth.format, response).into_response()
}
