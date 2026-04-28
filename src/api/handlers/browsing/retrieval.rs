//! Entity retrieval handlers (album, artist, song).

use axum::response::IntoResponse;

use crate::api::auth::SubsonicContext;
use crate::api::error::ApiError;
use crate::api::handlers::browsing::IdParams;
use crate::api::response::{SubsonicResponse, error_response};
use crate::models::music::{
    AlbumID3Response, AlbumWithSongsID3Response, ArtistWithAlbumsID3Response, ChildResponse,
};

fn album_response(
    auth: &SubsonicContext,
    album_id: i32,
) -> Result<AlbumWithSongsID3Response, ApiError> {
    let album = auth
        .music()
        .get_album(album_id)
        .map_err(|error| ApiError::Generic(error.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Album".into()))?;
    let album_starred_at = auth
        .music()
        .get_starred_at_for_album(auth.user.id, album_id)
        .map_err(|error| ApiError::Generic(error.to_string()))?;
    let songs = auth
        .music()
        .get_songs_by_album(album_id)
        .map_err(|error| ApiError::Generic(error.to_string()))?;
    let song_ids: Vec<i32> = songs.iter().map(|song| song.id).collect();
    let starred_songs = auth
        .music()
        .get_starred_at_for_songs_batch(auth.user.id, &song_ids)
        .map_err(|error| ApiError::Generic(error.to_string()))?;
    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|song| {
            let starred_at = starred_songs.get(&song.id);
            ChildResponse::from_song_with_starred(song, starred_at)
        })
        .collect();

    Ok(
        AlbumWithSongsID3Response::from_album_and_songs_with_starred(
            &album,
            song_responses,
            album_starred_at.as_ref(),
        ),
    )
}

fn artist_response(
    auth: &SubsonicContext,
    artist_id: i32,
) -> Result<ArtistWithAlbumsID3Response, ApiError> {
    let artist = auth
        .music()
        .get_artist(artist_id)
        .map_err(|error| ApiError::Generic(error.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Artist".into()))?;
    let artist_starred_at = auth
        .music()
        .get_starred_at_for_artist(auth.user.id, artist_id)
        .map_err(|error| ApiError::Generic(error.to_string()))?;
    let albums = auth
        .music()
        .get_albums_by_artist(artist_id)
        .map_err(|error| ApiError::Generic(error.to_string()))?;
    let album_ids: Vec<i32> = albums.iter().map(|album| album.id).collect();
    let starred_map = auth
        .music()
        .get_starred_at_for_albums_batch(auth.user.id, &album_ids)
        .map_err(|error| ApiError::Generic(error.to_string()))?;
    let album_responses: Vec<AlbumID3Response> = albums
        .iter()
        .map(|album| {
            let starred_at = starred_map.get(&album.id);
            AlbumID3Response::from_album_with_starred(album, starred_at)
        })
        .collect();

    Ok(
        ArtistWithAlbumsID3Response::from_artist_and_albums_with_starred(
            &artist,
            album_responses,
            artist_starred_at.as_ref(),
        ),
    )
}

fn song_response(auth: &SubsonicContext, song_id: i32) -> Result<ChildResponse, ApiError> {
    let song = auth
        .music()
        .get_song(song_id)
        .map_err(|error| ApiError::Generic(error.to_string()))?
        .ok_or_else(|| ApiError::NotFound("Song".into()))?;
    let starred_at = auth
        .music()
        .get_starred_at_for_song(auth.user.id, song_id)
        .map_err(|error| ApiError::Generic(error.to_string()))?;

    Ok(ChildResponse::from_song_with_starred(
        &song,
        starred_at.as_ref(),
    ))
}

/// GET/POST /rest/getAlbum[.view]
///
/// Returns details for an album, including its songs.
pub async fn get_album(
    axum::extract::Query(params): axum::extract::Query<IdParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(album_id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    let response = match album_response(&auth, album_id) {
        Ok(response) => response,
        Err(error) => return error_response(auth.format, &error).into_response(),
    };
    SubsonicResponse::album(auth.format, response).into_response()
}

/// GET/POST /rest/getArtist[.view]
///
/// Returns details for an artist, including their albums.
pub async fn get_artist(
    axum::extract::Query(params): axum::extract::Query<IdParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(artist_id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    let response = match artist_response(&auth, artist_id) {
        Ok(response) => response,
        Err(error) => return error_response(auth.format, &error).into_response(),
    };
    SubsonicResponse::artist(auth.format, response).into_response()
}

/// GET/POST /rest/getSong[.view]
///
/// Returns details for a song.
pub async fn get_song(
    axum::extract::Query(params): axum::extract::Query<IdParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(song_id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    let response = match song_response(&auth, song_id) {
        Ok(response) => response,
        Err(error) => return error_response(auth.format, &error).into_response(),
    };
    SubsonicResponse::song(auth.format, response).into_response()
}
