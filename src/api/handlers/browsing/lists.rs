//! List-based browsing handlers (albums, genres, songs).

use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;
use crate::api::response::{
    error_response, ok_album_list, ok_album_list2, ok_genres, ok_random_songs, ok_similar_songs,
    ok_similar_songs2, ok_songs_by_genre, ok_starred, ok_top_songs,
};
use crate::models::music::{
    AlbumID3Response, AlbumList2Response, AlbumListResponse, ArtistResponse, ChildResponse,
    GenreResponse, GenresResponse, RandomSongsResponse, SimilarSongs2Response,
    SimilarSongsResponse, SongsByGenreResponse, StarredResponse, TopSongsResponse,
};

/// Query parameters for getAlbumList2.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AlbumList2Params {
    /// The list type. Required.
    #[serde(rename = "type")]
    pub list_type: Option<String>,
    /// The number of albums to return. Default 10, max 500.
    pub size: Option<i64>,
    /// The list offset. Default 0.
    pub offset: Option<i64>,
    /// The first year in the range (for byYear type).
    #[serde(rename = "fromYear")]
    pub from_year: Option<i32>,
    /// The last year in the range (for byYear type).
    #[serde(rename = "toYear")]
    pub to_year: Option<i32>,
    /// The genre (for byGenre type).
    pub genre: Option<String>,
    /// Only return albums in this music folder.
    #[serde(rename = "musicFolderId")]
    pub music_folder_id: Option<i32>,
}

/// GET/POST /rest/getAlbumList2[.view]
///
/// Returns a list of random, newest, highest rated etc. albums.
/// Similar to getAlbumList, but organizes music according to ID3 tags.
pub async fn get_album_list2(
    axum::extract::Query(params): axum::extract::Query<AlbumList2Params>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(list_type) = params.list_type.as_deref() else {
        return error_response(auth.format, &ApiError::MissingParameter("type".into()))
            .into_response();
    };

    let size = params.size.unwrap_or(10).clamp(1, 500);
    let offset = params.offset.unwrap_or(0).max(0);

    let user_id = auth.user.id;
    let albums = match list_type {
        "random" => auth.state.get_albums_random(size),
        "newest" => auth.state.get_albums_newest(offset, size),
        "frequent" => auth.state.get_albums_frequent(offset, size),
        "recent" => auth.state.get_albums_recent(offset, size),
        "alphabeticalByName" => auth.state.get_albums_alphabetical_by_name(offset, size),
        "alphabeticalByArtist" => auth.state.get_albums_alphabetical_by_artist(offset, size),
        "byYear" => {
            let from_year = params.from_year.unwrap_or(0);
            let to_year = params.to_year.unwrap_or(9999);
            auth.state
                .get_albums_by_year(from_year, to_year, offset, size)
        }
        "byGenre" => {
            let Some(genre) = params.genre.as_deref() else {
                return error_response(auth.format, &ApiError::MissingParameter("genre".into()))
                    .into_response();
            };
            auth.state.get_albums_by_genre(genre, offset, size)
        }
        "starred" => auth.state.get_albums_starred(user_id, offset, size),
        "highest" => auth.state.get_albums_highest(user_id, offset, size),
        _ => {
            return error_response(
                auth.format,
                &ApiError::Generic(format!("Unknown list type: {list_type}")),
            )
            .into_response();
        }
    };

    // Batch fetch starred status for all albums
    let album_ids: Vec<i32> = albums.iter().map(|a| a.id).collect();
    let starred_map = auth
        .state
        .get_starred_at_for_albums_batch(user_id, &album_ids);

    let album_responses: Vec<AlbumID3Response> = albums
        .iter()
        .map(|a| {
            let starred_at = starred_map.get(&a.id);
            AlbumID3Response::from_album_with_starred(a, starred_at)
        })
        .collect();
    let response = AlbumList2Response {
        albums: album_responses,
    };

    ok_album_list2(auth.format, response).into_response()
}

/// GET/POST /rest/getAlbumList[.view]
///
/// Returns a list of random, newest, highest rated etc. albums.
/// This is the non-ID3 version that returns Child elements.
pub async fn get_album_list(
    axum::extract::Query(params): axum::extract::Query<AlbumList2Params>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(list_type) = params.list_type.as_deref() else {
        return error_response(auth.format, &ApiError::MissingParameter("type".into()))
            .into_response();
    };

    let size = params.size.unwrap_or(10).clamp(1, 500);
    let offset = params.offset.unwrap_or(0).max(0);

    let user_id = auth.user.id;
    let albums = match list_type {
        "random" => auth.state.get_albums_random(size),
        "newest" => auth.state.get_albums_newest(offset, size),
        "frequent" => auth.state.get_albums_frequent(offset, size),
        "recent" => auth.state.get_albums_recent(offset, size),
        "alphabeticalByName" => auth.state.get_albums_alphabetical_by_name(offset, size),
        "alphabeticalByArtist" => auth.state.get_albums_alphabetical_by_artist(offset, size),
        "byYear" => {
            let from_year = params.from_year.unwrap_or(0);
            let to_year = params.to_year.unwrap_or(9999);
            auth.state
                .get_albums_by_year(from_year, to_year, offset, size)
        }
        "byGenre" => {
            let Some(genre) = params.genre.as_deref() else {
                return error_response(auth.format, &ApiError::MissingParameter("genre".into()))
                    .into_response();
            };
            auth.state.get_albums_by_genre(genre, offset, size)
        }
        "starred" => auth.state.get_albums_starred(user_id, offset, size),
        "highest" => auth.state.get_albums_highest(user_id, offset, size),
        _ => {
            return error_response(
                auth.format,
                &ApiError::Generic(format!("Unknown list type: {list_type}")),
            )
            .into_response();
        }
    };

    // Batch fetch starred status for all albums
    let album_ids: Vec<i32> = albums.iter().map(|a| a.id).collect();
    let starred_map = auth
        .state
        .get_starred_at_for_albums_batch(user_id, &album_ids);

    // Convert to Child elements (non-ID3)
    let album_responses: Vec<ChildResponse> = albums
        .iter()
        .map(|a| {
            let starred_at = starred_map.get(&a.id);
            let mut response = ChildResponse::from_album_as_dir(a);
            if let Some(dt) = starred_at {
                response.starred = Some(dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string());
            }
            response
        })
        .collect();

    let response = AlbumListResponse {
        albums: album_responses,
    };

    ok_album_list(auth.format, response).into_response()
}

/// GET/POST /rest/getGenres[.view]
///
/// Returns all genres.
pub async fn get_genres(auth: SubsonicAuth) -> impl IntoResponse {
    let genres = auth.state.get_genres();
    let genre_responses: Vec<GenreResponse> = genres
        .into_iter()
        .map(|(name, song_count, album_count)| GenreResponse {
            value: name,
            song_count,
            album_count,
        })
        .collect();

    let response = GenresResponse {
        genres: genre_responses,
    };

    ok_genres(auth.format, response)
}

/// Query parameters for getRandomSongs.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct RandomSongsParams {
    /// The number of songs to return. Default 10, max 500.
    pub size: Option<i64>,
    /// Only returns songs belonging to this genre.
    pub genre: Option<String>,
    /// Only return songs published after or in this year.
    #[serde(rename = "fromYear")]
    pub from_year: Option<i32>,
    /// Only return songs published before or in this year.
    #[serde(rename = "toYear")]
    pub to_year: Option<i32>,
    /// Only return songs in this music folder.
    #[serde(rename = "musicFolderId")]
    pub music_folder_id: Option<i32>,
}

/// GET/POST /rest/getRandomSongs[.view]
///
/// Returns random songs matching the given criteria.
pub async fn get_random_songs(
    axum::extract::Query(params): axum::extract::Query<RandomSongsParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let size = params.size.unwrap_or(10).clamp(1, 500);
    let user_id = auth.user.id;

    let songs = auth.state.get_random_songs(
        size,
        params.genre.as_deref(),
        params.from_year,
        params.to_year,
        params.music_folder_id,
    );

    // Batch fetch starred status for all songs
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();
    let starred_songs = auth
        .state
        .get_starred_at_for_songs_batch(user_id, &song_ids);

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|s| {
            let starred_at = starred_songs.get(&s.id);
            ChildResponse::from_song_with_starred(s, starred_at)
        })
        .collect();

    let response = RandomSongsResponse {
        songs: song_responses,
    };

    ok_random_songs(auth.format, response)
}

/// Query parameters for getSongsByGenre.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SongsByGenreParams {
    /// The genre. Required.
    pub genre: Option<String>,
    /// The number of songs to return. Default 10, max 500.
    pub count: Option<i64>,
    /// The offset. Default 0.
    pub offset: Option<i64>,
    /// Only return songs in this music folder.
    #[serde(rename = "musicFolderId")]
    pub music_folder_id: Option<i32>,
}

/// GET/POST /rest/getSongsByGenre[.view]
///
/// Returns songs in a given genre.
pub async fn get_songs_by_genre(
    axum::extract::Query(params): axum::extract::Query<SongsByGenreParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(genre) = params.genre.as_deref() else {
        return error_response(auth.format, &ApiError::MissingParameter("genre".into()))
            .into_response();
    };

    let count = params.count.unwrap_or(10).clamp(1, 500);
    let offset = params.offset.unwrap_or(0).max(0);
    let user_id = auth.user.id;

    let songs = auth
        .state
        .get_songs_by_genre(genre, count, offset, params.music_folder_id);

    // Batch fetch starred status for all songs
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();
    let starred_songs = auth
        .state
        .get_starred_at_for_songs_batch(user_id, &song_ids);

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|s| {
            let starred_at = starred_songs.get(&s.id);
            ChildResponse::from_song_with_starred(s, starred_at)
        })
        .collect();

    let response = SongsByGenreResponse {
        songs: song_responses,
    };

    ok_songs_by_genre(auth.format, response).into_response()
}

/// Query parameters for getTopSongs.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct TopSongsParams {
    /// The artist name.
    pub artist: Option<String>,
    /// Max number of songs to return. Default 50.
    pub count: Option<i64>,
}

/// GET/POST /rest/getTopSongs[.view]
///
/// Returns the top songs for a given artist, ordered by play count.
pub async fn get_top_songs(
    axum::extract::Query(params): axum::extract::Query<TopSongsParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Get the required 'artist' parameter
    let artist_name = match params.artist.as_ref() {
        Some(name) if !name.is_empty() => name,
        _ => {
            return error_response(auth.format, &ApiError::MissingParameter("artist".into()))
                .into_response();
        }
    };

    let count = params.count.unwrap_or(50).clamp(1, 500);
    let user_id = auth.user.id;

    // Get top songs by artist name (ordered by play count)
    let songs = auth.state.get_top_songs_by_artist_name(artist_name, count);

    // Batch fetch starred status for all songs
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();
    let starred_songs = auth
        .state
        .get_starred_at_for_songs_batch(user_id, &song_ids);

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|s| {
            let starred_at = starred_songs.get(&s.id);
            ChildResponse::from_song_with_starred(s, starred_at)
        })
        .collect();

    let response = TopSongsResponse {
        songs: song_responses,
    };

    ok_top_songs(auth.format, response).into_response()
}

/// Query parameters for getSimilarSongs2.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SimilarSongs2Params {
    /// The song/album/artist ID.
    pub id: Option<String>,
    /// Max number of similar songs to return. Default 50.
    pub count: Option<i64>,
}

/// GET/POST /rest/getSimilarSongs2[.view]
///
/// Returns songs similar to the given song, album, or artist.
/// Since we don't have external metadata, we return random songs from the same artist or genre.
pub async fn get_similar_songs2(
    axum::extract::Query(params): axum::extract::Query<SimilarSongs2Params>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Get the required 'id' parameter
    let Some(id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    let count = params.count.unwrap_or(50).clamp(1, 500);
    let user_id = auth.user.id;

    // Try to get similar songs - first check if it's a song
    let songs = if let Some(song) = auth.state.get_song(id) {
        // Get songs from the same artist (excluding this song)
        if let Some(artist_id) = song.artist_id {
            auth.state.get_similar_songs_by_artist(artist_id, id, count)
        } else if let Some(ref genre) = song.genre {
            // Fall back to same genre
            auth.state.get_songs_by_genre(genre, count, 0, None)
        } else {
            Vec::new()
        }
    } else if let Some(album) = auth.state.get_album(id) {
        // Get songs from the same artist
        if let Some(artist_id) = album.artist_id {
            auth.state.get_similar_songs_by_artist(artist_id, -1, count)
        } else {
            Vec::new()
        }
    } else if auth.state.get_artist(id).is_some() {
        // Get random songs from this artist
        auth.state.get_similar_songs_by_artist(id, -1, count)
    } else {
        return error_response(auth.format, &ApiError::NotFound("Item".into())).into_response();
    };

    // Batch fetch starred status for all songs
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();
    let starred_songs = auth
        .state
        .get_starred_at_for_songs_batch(user_id, &song_ids);

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|s| {
            let starred_at = starred_songs.get(&s.id);
            ChildResponse::from_song_with_starred(s, starred_at)
        })
        .collect();

    let response = SimilarSongs2Response {
        songs: song_responses,
    };

    ok_similar_songs2(auth.format, response).into_response()
}

/// GET/POST /rest/getSimilarSongs[.view]
///
/// Returns similar songs (non-ID3 version). Similar to getSimilarSongs2.
pub async fn get_similar_songs(
    axum::extract::Query(params): axum::extract::Query<SimilarSongs2Params>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Get the required 'id' parameter
    let Some(id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    let count = params.count.unwrap_or(50).clamp(1, 500);
    let user_id = auth.user.id;

    // Try to get similar songs
    let songs = if let Some(song) = auth.state.get_song(id) {
        if let Some(artist_id) = song.artist_id {
            auth.state.get_similar_songs_by_artist(artist_id, id, count)
        } else if let Some(ref genre) = song.genre {
            auth.state.get_songs_by_genre(genre, count, 0, None)
        } else {
            Vec::new()
        }
    } else if let Some(album) = auth.state.get_album(id) {
        if let Some(artist_id) = album.artist_id {
            auth.state.get_similar_songs_by_artist(artist_id, -1, count)
        } else {
            Vec::new()
        }
    } else if auth.state.get_artist(id).is_some() {
        auth.state.get_similar_songs_by_artist(id, -1, count)
    } else {
        return error_response(auth.format, &ApiError::NotFound("Item".into())).into_response();
    };

    // Batch fetch starred status for all songs
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();
    let starred_songs = auth
        .state
        .get_starred_at_for_songs_batch(user_id, &song_ids);

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|s| {
            let starred_at = starred_songs.get(&s.id);
            ChildResponse::from_song_with_starred(s, starred_at)
        })
        .collect();

    let response = SimilarSongsResponse {
        songs: song_responses,
    };

    ok_similar_songs(auth.format, response).into_response()
}

/// GET/POST /rest/getStarred[.view]
///
/// Returns starred songs, albums and artists (non-ID3 version).
pub async fn get_starred(auth: SubsonicAuth) -> impl IntoResponse {
    let user_id = auth.user.id;

    // Get starred items
    let starred_artists = auth.state.get_starred_artists(user_id);
    let starred_albums = auth.state.get_starred_albums(user_id);
    let starred_songs = auth.state.get_starred_songs(user_id);

    // Convert to response types
    let artist_responses: Vec<ArtistResponse> = starred_artists
        .iter()
        .map(|(artist, starred_at)| {
            ArtistResponse::from_artist_with_starred(artist, Some(starred_at))
        })
        .collect();

    let album_responses: Vec<ChildResponse> = starred_albums
        .iter()
        .map(|(album, starred_at)| {
            let mut response = ChildResponse::from_album_as_dir(album);
            response.starred = Some(starred_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string());
            response
        })
        .collect();

    let song_responses: Vec<ChildResponse> = starred_songs
        .iter()
        .map(|(song, starred_at)| ChildResponse::from_song_with_starred(song, Some(starred_at)))
        .collect();

    let response = StarredResponse {
        artists: artist_responses,
        albums: album_responses,
        songs: song_responses,
    };

    ok_starred(auth.format, response)
}
