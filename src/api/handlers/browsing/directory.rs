//! Directory browsing handlers.

use axum::response::IntoResponse;

use crate::api::auth::SubsonicContext;
use crate::api::handlers::browsing::IdParams;
use crate::api::handlers::util;
use crate::api::response::SubsonicResponse;
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
    use crate::models::music::EntityId;

    // Get the required 'id' parameter
    let Some(id_str) = params.id.as_deref() else {
        return util::missing_param(&auth, "id");
    };
    let Some(entity_id) = EntityId::parse(id_str) else {
        return util::service_error(&auth, format!("Invalid id: {id_str}"));
    };

    match entity_id {
        EntityId::Album(id) => return album_directory(&auth, id),
        EntityId::Artist(id) => return artist_directory(&auth, id),
        EntityId::Song(_) if id_str.starts_with("mf-") => {
            return util::not_found(&auth, "Directory");
        }
        EntityId::Song(id) => {
            // Bare integer: fall back to album/artist/folder precedence below
            let folders = match auth.music().get_music_folders() {
                Ok(folders) => folders,
                Err(e) => {
                    return util::repo_error(&auth, e);
                }
            };

            let maybe_album = match auth.music().get_album(id) {
                Ok(album) => album,
                Err(e) => {
                    return util::repo_error(&auth, e);
                }
            };
            if maybe_album.is_some() {
                return album_directory(&auth, id);
            }

            let maybe_artist = match auth.music().get_artist(id) {
                Ok(artist) => artist,
                Err(e) => {
                    return util::repo_error(&auth, e);
                }
            };
            if maybe_artist.is_some() {
                return artist_directory(&auth, id);
            }

            if let Some(folder) = folders.iter().find(|f| f.id == id) {
                let artists = match auth.music().get_artists_by_music_folder(folder.id) {
                    Ok(artists) => artists,
                    Err(e) => {
                        return util::repo_error(&auth, e);
                    }
                };
                let children: Vec<ChildResponse> = artists
                    .iter()
                    .map(ChildResponse::from_artist_as_dir)
                    .collect();
                let response = DirectoryResponse::from_music_folder(folder, children);
                return SubsonicResponse::directory(auth.format, response).into_response();
            }
        }
        EntityId::Playlist(_) => {}
    }

    util::not_found(&auth, "Directory")
}

/// Serve a directory listing for an album: its songs.
fn album_directory(auth: &SubsonicContext, album_id: i32) -> axum::response::Response {
    let album = match auth.music().get_album(album_id) {
        Ok(Some(album)) => album,
        Ok(None) => return util::not_found(auth, "Directory"),
        Err(e) => {
            return util::repo_error(auth, e);
        }
    };
    let songs = match auth.music().get_songs_by_album(album_id) {
        Ok(songs) => songs,
        Err(e) => {
            return util::repo_error(auth, e);
        }
    };
    let annotated = match auth.music().annotate_songs_for_user(auth.user.id, songs) {
        Ok(annotated) => annotated,
        Err(e) => {
            return util::repo_error(auth, e);
        }
    };
    let response = DirectoryResponse::from_album(&album, util::annotate_songs(annotated));
    SubsonicResponse::directory(auth.format, response).into_response()
}

/// Serve a directory listing for an artist: their albums as subdirectories.
fn artist_directory(auth: &SubsonicContext, artist_id: i32) -> axum::response::Response {
    let artist = match auth.music().get_artist(artist_id) {
        Ok(Some(artist)) => artist,
        Ok(None) => return util::not_found(auth, "Directory"),
        Err(e) => {
            return util::repo_error(auth, e);
        }
    };
    let albums = match auth.music().get_albums_by_artist(artist_id) {
        Ok(albums) => albums,
        Err(e) => {
            return util::repo_error(auth, e);
        }
    };
    let children: Vec<ChildResponse> = albums
        .iter()
        .map(ChildResponse::from_album_as_dir)
        .collect();
    let response = DirectoryResponse::from_artist(&artist, children);
    SubsonicResponse::directory(auth.format, response).into_response()
}
