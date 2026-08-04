//! Entity retrieval handlers (album, artist, song).

use axum::response::IntoResponse;

use crate::api::auth::SubsonicContext;
use crate::api::error::ApiError;
use crate::api::handlers::browsing::IdParams;
use crate::api::handlers::util;
use crate::api::response::SubsonicResponse;
use crate::models::music::{AlbumWithSongsID3Response, ArtistWithAlbumsID3Response, ChildResponse};

fn album_response(
    auth: &SubsonicContext,
    album_id: i32,
) -> Result<AlbumWithSongsID3Response, ApiError> {
    let annotated_album = auth
        .music()
        .annotated_album_for_user(auth.user.id, album_id)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound("Album".into()))?;
    let songs = auth
        .music()
        .get_songs_by_album(album_id)
        .map_err(ApiError::from)?;
    let annotated = auth
        .music()
        .annotate_songs_for_user(auth.user.id, songs)
        .map_err(ApiError::from)?;

    Ok(
        AlbumWithSongsID3Response::from_album_and_songs_with_starred(
            &annotated_album.album,
            util::annotate_songs(annotated),
            annotated_album.starred_at.as_ref(),
        )
        .with_annotations(Some(&annotated_album.annotations)),
    )
}

fn artist_response(
    auth: &SubsonicContext,
    artist_id: i32,
) -> Result<ArtistWithAlbumsID3Response, ApiError> {
    let artist = auth
        .music()
        .get_artist(artist_id)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound("Artist".into()))?;
    let artist_starred_at = auth
        .music()
        .get_starred_at_for_artist(auth.user.id, artist_id)
        .map_err(ApiError::from)?;
    let albums = auth
        .music()
        .get_albums_by_artist(artist_id)
        .map_err(ApiError::from)?;
    let annotated = auth
        .music()
        .annotate_albums_for_user(auth.user.id, albums)
        .map_err(ApiError::from)?;

    Ok(
        ArtistWithAlbumsID3Response::from_artist_and_albums_with_starred(
            &artist,
            util::annotate_albums(annotated),
            artist_starred_at.as_ref(),
        ),
    )
}

fn song_response(auth: &SubsonicContext, song_id: i32) -> Result<ChildResponse, ApiError> {
    let annotated = auth
        .music()
        .annotated_song_for_user(auth.user.id, song_id)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::NotFound("Song".into()))?;

    Ok(ChildResponse::from_song_with_starred(
        &annotated.song,
        annotated.annotations.starred_at.as_ref(),
    )
    .with_annotations(Some(&annotated.annotations)))
}

/// GET/POST /rest/getAlbum[.view]
///
/// Returns details for an album, including its songs.
pub async fn get_album(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<IdParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(album_id) = params
        .id
        .as_deref()
        .and_then(crate::models::music::EntityId::parse_album)
    else {
        return util::missing_param(&auth, "id");
    };

    let response = match album_response(&auth, album_id) {
        Ok(response) => response,
        Err(error) => return util::api_error(&auth, &error),
    };
    SubsonicResponse::album(auth.format, response).into_response()
}

/// GET/POST /rest/getArtist[.view]
///
/// Returns details for an artist, including their albums.
pub async fn get_artist(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<IdParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(artist_id) = params
        .id
        .as_deref()
        .and_then(crate::models::music::EntityId::parse_artist)
    else {
        return util::missing_param(&auth, "id");
    };

    let response = match artist_response(&auth, artist_id) {
        Ok(response) => response,
        Err(error) => return util::api_error(&auth, &error),
    };
    SubsonicResponse::artist(auth.format, response).into_response()
}

/// GET/POST /rest/getSong[.view]
///
/// Returns details for a song.
pub async fn get_song(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<IdParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(song_id) = params
        .id
        .as_deref()
        .and_then(crate::models::music::EntityId::parse_song)
    else {
        return util::missing_param(&auth, "id");
    };

    let response = match song_response(&auth, song_id) {
        Ok(response) => response,
        Err(error) => return util::api_error(&auth, &error),
    };
    SubsonicResponse::song(auth.format, response).into_response()
}
