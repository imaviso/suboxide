//! Entity retrieval handlers (album, artist, song).

use axum::response::IntoResponse;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;
use crate::api::handlers::browsing::IdParams;
use crate::api::response::{error_response, ok_album, ok_artist, ok_song};
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
    // Get the required 'id' parameter
    let Some(album_id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Get the album
    let Some(album) = auth.state.get_album(album_id) else {
        return error_response(auth.format, &ApiError::NotFound("Album".into())).into_response();
    };

    // Get the album's starred status
    let album_starred_at = auth.state.get_starred_at_for_album(auth.user.id, album_id);

    // Get songs for the album
    let songs = auth.state.get_songs_by_album(album_id);

    // Get starred status for all songs in a single batch query
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();
    let starred_songs = auth
        .state
        .get_starred_at_for_songs_batch(auth.user.id, &song_ids);

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
    ok_album(auth.format, response).into_response()
}

/// GET/POST /rest/getArtist[.view]
///
/// Returns details for an artist, including their albums.
pub async fn get_artist(
    axum::extract::Query(params): axum::extract::Query<IdParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Get the required 'id' parameter
    let Some(artist_id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Get the artist
    let Some(artist) = auth.state.get_artist(artist_id) else {
        return error_response(auth.format, &ApiError::NotFound("Artist".into())).into_response();
    };

    // Get the artist's starred status
    let artist_starred_at = auth
        .state
        .get_starred_at_for_artist(auth.user.id, artist_id);

    // Get albums for the artist with their starred status (batch lookup)
    let albums = auth.state.get_albums_by_artist(artist_id);
    let album_ids: Vec<i32> = albums.iter().map(|a| a.id).collect();
    let starred_map = auth
        .state
        .get_starred_at_for_albums_batch(auth.user.id, &album_ids);

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
    ok_artist(auth.format, response).into_response()
}

/// GET/POST /rest/getSong[.view]
///
/// Returns details for a song.
pub async fn get_song(
    axum::extract::Query(params): axum::extract::Query<IdParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Get the required 'id' parameter
    let Some(song_id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Get the song
    let Some(song) = auth.state.get_song(song_id) else {
        return error_response(auth.format, &ApiError::NotFound("Song".into())).into_response();
    };

    // Get the song's starred status
    let starred_at = auth.state.get_starred_at_for_song(auth.user.id, song_id);
    let response = ChildResponse::from_song_with_starred(&song, starred_at.as_ref());
    ok_song(auth.format, response).into_response()
}
