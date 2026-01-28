//! Browsing-related API handlers (getMusicFolders, getArtists, etc.)
#![allow(clippy::unused_async)]

use std::collections::BTreeMap;

use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;
use crate::api::response::{
    error_response, ok_album, ok_album_info, ok_album_list, ok_album_list2, ok_artist,
    ok_artist_info, ok_artist_info2, ok_artists, ok_directory, ok_genres, ok_indexes, ok_lyrics,
    ok_lyrics_list, ok_music_folders, ok_random_songs, ok_search_result, ok_search_result2,
    ok_search_result3, ok_similar_songs, ok_similar_songs2, ok_song, ok_songs_by_genre, ok_starred,
    ok_top_songs,
};
use crate::models::music::{
    AlbumID3Response, AlbumInfoResponse, AlbumList2Response, AlbumListResponse,
    AlbumWithSongsID3Response, ArtistID3Response, ArtistInfo2Response, ArtistInfoResponse,
    ArtistResponse, ArtistWithAlbumsID3Response, ArtistsID3Response, ChildResponse,
    DirectoryResponse, GenreResponse, GenresResponse, IndexID3Response, IndexResponse,
    IndexesResponse, LyricLine, LyricsListResponse, LyricsResponse, MusicFolderResponse,
    RandomSongsResponse, SearchMatch, SearchResult2Response, SearchResult3Response,
    SearchResultResponse, SimilarSongs2Response, SimilarSongsResponse, SongsByGenreResponse,
    StarredResponse, StructuredLyrics, TopSongsResponse,
};

/// Query parameters for endpoints that require an ID.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct IdParams {
    /// The ID of the item to retrieve.
    pub id: Option<String>,
}

/// GET/POST /rest/getMusicFolders[.view]
///
/// Returns all configured top-level music folders.
pub async fn get_music_folders(auth: SubsonicAuth) -> impl IntoResponse {
    let folders = auth.state.get_music_folders();
    let responses: Vec<MusicFolderResponse> =
        folders.iter().map(MusicFolderResponse::from).collect();
    ok_music_folders(auth.format, responses)
}

/// GET/POST /rest/getIndexes[.view]
///
/// Returns an indexed structure of all artists.
/// This is used by older clients that use the folder-based browsing model.
pub async fn get_indexes(auth: SubsonicAuth) -> impl IntoResponse {
    let artists = auth.state.get_artists();

    // Group artists by first letter
    let mut index_map: BTreeMap<String, Vec<ArtistResponse>> = BTreeMap::new();

    for artist in &artists {
        let first_char = artist
            .sort_name
            .as_ref()
            .unwrap_or(&artist.name)
            .chars()
            .next()
            .unwrap_or('#')
            .to_uppercase()
            .next()
            .unwrap_or('#');

        let key = if first_char.is_alphabetic() {
            first_char.to_string()
        } else {
            "#".to_string()
        };

        index_map
            .entry(key)
            .or_default()
            .push(ArtistResponse::from(artist));
    }

    // Convert to response format
    let indexes: Vec<IndexResponse> = index_map
        .into_iter()
        .map(|(name, artists)| IndexResponse { name, artists })
        .collect();

    // Get last modified time (using current timestamp for now)
    let last_modified = auth
        .state
        .get_artists_last_modified()
        .map_or(0, |dt| dt.and_utc().timestamp_millis());

    let response = IndexesResponse {
        ignored_articles: "The El La Los Las Le Les".to_string(),
        last_modified,
        indexes,
    };

    ok_indexes(auth.format, response)
}

/// GET/POST /rest/getArtists[.view]
///
/// Similar to getIndexes, but returns artists using ID3 tags.
/// This is the preferred endpoint for modern clients.
pub async fn get_artists(auth: SubsonicAuth) -> impl IntoResponse {
    let artists = auth.state.get_artists();

    // Get album counts for all artists in a single batch query
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let album_counts = auth.state.get_artist_album_counts_batch(&artist_ids);

    // Group artists by first letter
    let mut index_map: BTreeMap<String, Vec<ArtistID3Response>> = BTreeMap::new();

    for artist in &artists {
        let first_char = artist
            .sort_name
            .as_ref()
            .unwrap_or(&artist.name)
            .chars()
            .next()
            .unwrap_or('#')
            .to_uppercase()
            .next()
            .unwrap_or('#');

        let key = if first_char.is_alphabetic() {
            first_char.to_string()
        } else {
            "#".to_string()
        };

        // Get album count from batch result
        let album_count = album_counts.get(&artist.id).copied().unwrap_or(0);

        index_map
            .entry(key)
            .or_default()
            .push(ArtistID3Response::from_artist(
                artist,
                Some(i32::try_from(album_count).unwrap_or(0)),
            ));
    }

    // Convert to response format
    let indexes: Vec<IndexID3Response> = index_map
        .into_iter()
        .map(|(name, artists)| IndexID3Response { name, artists })
        .collect();

    let response = ArtistsID3Response {
        ignored_articles: "The El La Los Las Le Les".to_string(),
        indexes,
    };

    ok_artists(auth.format, response)
}

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

// ============================================================================
// Album List, Genres, and Search endpoints
// ============================================================================

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
                return error_response(
                    auth.format,
                    &ApiError::MissingParameter("genre".into()),
                )
                .into_response();
            };
            auth.state.get_albums_by_genre(genre, offset, size)
        }
        "starred" => auth.state.get_albums_starred(auth.user.id, offset, size),
        "highest" => auth.state.get_albums_highest(auth.user.id, offset, size),
        _ => {
            return error_response(
                auth.format,
                &ApiError::Generic(format!("Unknown list type: {list_type}")),
            )
            .into_response();
        }
    };

    let album_responses: Vec<AlbumID3Response> =
        albums.iter().map(AlbumID3Response::from).collect();
    let response = AlbumList2Response {
        albums: album_responses,
    };

    ok_album_list2(auth.format, response).into_response()
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

/// Query parameters for search3.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Search3Params {
    /// Search query.
    pub query: Option<String>,
    /// Maximum number of artists to return. Default 20.
    #[serde(rename = "artistCount")]
    pub artist_count: Option<i64>,
    /// Artist search result offset. Default 0.
    #[serde(rename = "artistOffset")]
    pub artist_offset: Option<i64>,
    /// Maximum number of albums to return. Default 20.
    #[serde(rename = "albumCount")]
    pub album_count: Option<i64>,
    /// Album search result offset. Default 0.
    #[serde(rename = "albumOffset")]
    pub album_offset: Option<i64>,
    /// Maximum number of songs to return. Default 20.
    #[serde(rename = "songCount")]
    pub song_count: Option<i64>,
    /// Song search result offset. Default 0.
    #[serde(rename = "songOffset")]
    pub song_offset: Option<i64>,
    /// Only return results from this music folder.
    #[serde(rename = "musicFolderId")]
    pub music_folder_id: Option<i32>,
}

/// GET/POST /rest/search3[.view]
///
/// Returns albums, artists and songs matching the given search criteria.
/// Supports paging through the result.
/// An empty query returns all results (up to the count limits).
pub async fn search3(
    axum::extract::Query(params): axum::extract::Query<Search3Params>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Empty query is allowed - it returns all results
    // Some clients send "" (quoted empty string) which we need to handle
    let raw_query = params.query.as_deref().unwrap_or("");
    let query = raw_query.trim_matches('"').trim();

    let artist_count = params.artist_count.unwrap_or(20).clamp(0, 500);
    let artist_offset = params.artist_offset.unwrap_or(0).max(0);
    let album_count = params.album_count.unwrap_or(20).clamp(0, 500);
    let album_offset = params.album_offset.unwrap_or(0).max(0);
    let song_count = params.song_count.unwrap_or(20).clamp(0, 500);
    let song_offset = params.song_offset.unwrap_or(0).max(0);

    // Search for artists, albums, and songs
    let artists = auth
        .state
        .search_artists(query, artist_offset, artist_count);
    let albums = auth.state.search_albums(query, album_offset, album_count);
    let songs = auth.state.search_songs(query, song_offset, song_count);

    // Collect IDs for batch queries
    let user_id = auth.user.id;
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let album_ids: Vec<i32> = albums.iter().map(|a| a.id).collect();
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();

    // Batch fetch album counts and starred status
    let artist_album_counts = auth.state.get_artist_album_counts_batch(&artist_ids);
    let starred_artists = auth
        .state
        .get_starred_at_for_artists_batch(user_id, &artist_ids);
    let starred_albums = auth
        .state
        .get_starred_at_for_albums_batch(user_id, &album_ids);
    let starred_songs = auth
        .state
        .get_starred_at_for_songs_batch(user_id, &song_ids);

    // Convert to response types with starred status from batch results
    let artist_responses: Vec<ArtistID3Response> = artists
        .iter()
        .map(|a| {
            let album_count = artist_album_counts.get(&a.id).copied().unwrap_or(0);
            let starred_at = starred_artists.get(&a.id);
            ArtistID3Response::from_artist_with_starred(
                a,
                Some(i32::try_from(album_count).unwrap_or(0)),
                starred_at,
            )
        })
        .collect();

    let album_responses: Vec<AlbumID3Response> = albums
        .iter()
        .map(|a| {
            let starred_at = starred_albums.get(&a.id);
            AlbumID3Response::from_album_with_starred(a, starred_at)
        })
        .collect();

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|s| {
            let starred_at = starred_songs.get(&s.id);
            ChildResponse::from_song_with_starred(s, starred_at)
        })
        .collect();

    let response = SearchResult3Response {
        artists: artist_responses,
        albums: album_responses,
        songs: song_responses,
    };

    ok_search_result3(auth.format, response).into_response()
}

// ============================================================================
// Random Songs and Songs by Genre endpoints
// ============================================================================

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

// ============================================================================
// Artist Info, Album Info, Similar Songs, and Top Songs endpoints
// ============================================================================

/// Query parameters for getArtistInfo2.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ArtistInfo2Params {
    /// The artist ID.
    pub id: Option<String>,
    /// Max number of similar artists to return.
    pub count: Option<i32>,
    /// Whether to include artists that are not present in the media library.
    #[serde(rename = "includeNotPresent")]
    pub include_not_present: Option<bool>,
}

/// GET/POST /rest/getArtistInfo2[.view]
///
/// Returns artist info with biography, image URLs, similar artists, etc.
/// This is a stub implementation that returns minimal data from the database.
pub async fn get_artist_info2(
    axum::extract::Query(params): axum::extract::Query<ArtistInfo2Params>,
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

    // Create response with available data from the artist
    let response = ArtistInfo2Response::from_artist(&artist);
    ok_artist_info2(auth.format, response).into_response()
}

/// Query parameters for getAlbumInfo2.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AlbumInfo2Params {
    /// The album ID.
    pub id: Option<String>,
}

/// GET/POST /rest/getAlbumInfo2[.view]
///
/// Returns album info with notes, `MusicBrainz` ID, image URLs, etc.
/// This is a stub implementation that returns minimal data from the database.
pub async fn get_album_info2(
    axum::extract::Query(params): axum::extract::Query<AlbumInfo2Params>,
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

    // Create response with available data from the album
    let response = AlbumInfoResponse::from_album(&album);
    ok_album_info(auth.format, response).into_response()
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

// ============================================================================
// Non-ID3 Browsing Endpoints (folder-based browsing for older clients)
// ============================================================================

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
    let Some(id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Try to find what this ID refers to: music folder, artist, or album
    // First, check if it's an album (most common case when browsing)
    if let Some(album) = auth.state.get_album(id) {
        let songs = auth.state.get_songs_by_album(id);
        let children: Vec<ChildResponse> = songs.iter().map(ChildResponse::from).collect();
        let response = DirectoryResponse::from_album(&album, children);
        return ok_directory(auth.format, response).into_response();
    }

    // Check if it's an artist
    if let Some(artist) = auth.state.get_artist(id) {
        let albums = auth.state.get_albums_by_artist(id);
        let children: Vec<ChildResponse> = albums
            .iter()
            .map(ChildResponse::from_album_as_dir)
            .collect();
        let response = DirectoryResponse::from_artist(&artist, children);
        return ok_directory(auth.format, response).into_response();
    }

    // Check if it's a music folder
    let folders = auth.state.get_music_folders();
    if let Some(folder) = folders.iter().find(|f| f.id == id) {
        // For music folders, return all artists as children
        let artists = auth.state.get_artists();
        let children: Vec<ChildResponse> = artists
            .iter()
            .map(ChildResponse::from_artist_as_dir)
            .collect();
        let response = DirectoryResponse::from_music_folder(folder, children);
        return ok_directory(auth.format, response).into_response();
    }

    error_response(auth.format, &ApiError::NotFound("Directory".into())).into_response()
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
                return error_response(
                    auth.format,
                    &ApiError::MissingParameter("genre".into()),
                )
                .into_response();
            };
            auth.state.get_albums_by_genre(genre, offset, size)
        }
        "starred" => auth.state.get_albums_starred(auth.user.id, offset, size),
        "highest" => auth.state.get_albums_highest(auth.user.id, offset, size),
        _ => {
            return error_response(
                auth.format,
                &ApiError::Generic(format!("Unknown list type: {list_type}")),
            )
            .into_response();
        }
    };

    // Convert to Child elements (non-ID3)
    let album_responses: Vec<ChildResponse> = albums
        .iter()
        .map(ChildResponse::from_album_as_dir)
        .collect();

    let response = AlbumListResponse {
        albums: album_responses,
    };

    ok_album_list(auth.format, response).into_response()
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

/// Query parameters for search2.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Search2Params {
    /// Search query.
    pub query: Option<String>,
    /// Maximum number of artists to return. Default 20.
    #[serde(rename = "artistCount")]
    pub artist_count: Option<i64>,
    /// Artist search result offset. Default 0.
    #[serde(rename = "artistOffset")]
    pub artist_offset: Option<i64>,
    /// Maximum number of albums to return. Default 20.
    #[serde(rename = "albumCount")]
    pub album_count: Option<i64>,
    /// Album search result offset. Default 0.
    #[serde(rename = "albumOffset")]
    pub album_offset: Option<i64>,
    /// Maximum number of songs to return. Default 20.
    #[serde(rename = "songCount")]
    pub song_count: Option<i64>,
    /// Song search result offset. Default 0.
    #[serde(rename = "songOffset")]
    pub song_offset: Option<i64>,
    /// Only return results from this music folder.
    #[serde(rename = "musicFolderId")]
    pub music_folder_id: Option<i32>,
}

/// GET/POST /rest/search2[.view]
///
/// Returns albums, artists and songs matching the given search criteria (non-ID3).
pub async fn search2(
    axum::extract::Query(params): axum::extract::Query<Search2Params>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let raw_query = params.query.as_deref().unwrap_or("");
    let query = raw_query.trim_matches('"').trim();

    let artist_count = params.artist_count.unwrap_or(20).clamp(0, 500);
    let artist_offset = params.artist_offset.unwrap_or(0).max(0);
    let album_count = params.album_count.unwrap_or(20).clamp(0, 500);
    let album_offset = params.album_offset.unwrap_or(0).max(0);
    let song_count = params.song_count.unwrap_or(20).clamp(0, 500);
    let song_offset = params.song_offset.unwrap_or(0).max(0);

    // Search for artists, albums, and songs
    let artists = auth
        .state
        .search_artists(query, artist_offset, artist_count);
    let albums = auth.state.search_albums(query, album_offset, album_count);
    let songs = auth.state.search_songs(query, song_offset, song_count);

    // Collect IDs for batch queries
    let user_id = auth.user.id;
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let album_ids: Vec<i32> = albums.iter().map(|a| a.id).collect();
    let song_ids: Vec<i32> = songs.iter().map(|s| s.id).collect();

    // Batch fetch starred status
    let starred_artists = auth
        .state
        .get_starred_at_for_artists_batch(user_id, &artist_ids);
    let starred_albums = auth
        .state
        .get_starred_at_for_albums_batch(user_id, &album_ids);
    let starred_songs = auth
        .state
        .get_starred_at_for_songs_batch(user_id, &song_ids);

    // Convert to non-ID3 response types
    let artist_responses: Vec<ArtistResponse> = artists
        .iter()
        .map(|a| {
            let starred_at = starred_artists.get(&a.id);
            ArtistResponse::from_artist_with_starred(a, starred_at)
        })
        .collect();

    let album_responses: Vec<ChildResponse> = albums
        .iter()
        .map(|a| {
            let starred_at = starred_albums.get(&a.id);
            let mut response = ChildResponse::from_album_as_dir(a);
            response.starred = starred_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string());
            response
        })
        .collect();

    let song_responses: Vec<ChildResponse> = songs
        .iter()
        .map(|s| {
            let starred_at = starred_songs.get(&s.id);
            ChildResponse::from_song_with_starred(s, starred_at)
        })
        .collect();

    let response = SearchResult2Response {
        artists: artist_responses,
        albums: album_responses,
        songs: song_responses,
    };

    ok_search_result2(auth.format, response).into_response()
}

/// Query parameters for legacy search.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SearchParams {
    /// Artist to search for.
    pub artist: Option<String>,
    /// Album to search for.
    pub album: Option<String>,
    /// Song title to search for.
    pub title: Option<String>,
    /// Searches all fields.
    pub any: Option<String>,
    /// Maximum number of results to return. Default 20.
    pub count: Option<i64>,
    /// Search result offset. Default 0.
    pub offset: Option<i64>,
    /// Only return matches that are newer than this timestamp.
    #[serde(rename = "newerThan")]
    pub newer_than: Option<i64>,
}

/// GET/POST /rest/search[.view]
///
/// Returns a listing of files matching the given search criteria (legacy).
pub async fn search(
    axum::extract::Query(params): axum::extract::Query<SearchParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let count = params.count.unwrap_or(20).clamp(0, 500);
    let offset = params.offset.unwrap_or(0).max(0);

    // Use 'any' field for general search, or combine artist/album/title
    let query = params
        .any
        .as_deref()
        .or(params.title.as_deref())
        .or(params.album.as_deref())
        .or(params.artist.as_deref())
        .unwrap_or("")
        .trim();

    // Search for songs (legacy search only returns songs as matches)
    let songs = auth.state.search_songs(query, offset, count);

    let matches: Vec<SearchMatch> = songs.iter().map(SearchMatch::from).collect();
    #[allow(clippy::cast_possible_wrap)]
    let total_hits = matches.len() as i64;

    let response = SearchResultResponse {
        offset,
        total_hits,
        matches,
    };

    ok_search_result(auth.format, response).into_response()
}

/// GET/POST /rest/getArtistInfo[.view]
///
/// Returns artist info (non-ID3 version). Similar to getArtistInfo2.
pub async fn get_artist_info(
    axum::extract::Query(params): axum::extract::Query<ArtistInfo2Params>,
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

    let response = ArtistInfoResponse::from_artist(&artist);
    ok_artist_info(auth.format, response).into_response()
}

/// GET/POST /rest/getAlbumInfo[.view]
///
/// Returns album info (non-ID3 version). Similar to getAlbumInfo2.
pub async fn get_album_info(
    axum::extract::Query(params): axum::extract::Query<AlbumInfo2Params>,
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

    // Use AlbumInfoResponse which is the same for ID3 and non-ID3
    let response = AlbumInfoResponse::from_album(&album);
    ok_album_info(auth.format, response).into_response()
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

// ============================================================================
// Lyrics Endpoints
// ============================================================================

/// Query parameters for getLyrics.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct LyricsParams {
    /// The artist name.
    pub artist: Option<String>,
    /// The song title.
    pub title: Option<String>,
}

/// GET/POST /rest/getLyrics[.view]
///
/// Searches for and returns lyrics for a given song.
/// Note: This is a stub implementation that returns empty lyrics since we don't
/// have a lyrics database or external service integration.
pub async fn get_lyrics(
    axum::extract::Query(params): axum::extract::Query<LyricsParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Return empty lyrics with the requested artist/title
    let response = LyricsResponse::new(
        params.artist.clone(),
        params.title,
        None, // No lyrics content
    );

    ok_lyrics(auth.format, response)
}

/// GET/POST /rest/getLyricsBySongId[.view]
///
/// Returns structured lyrics for a given song (`OpenSubsonic` extension).
/// Extracts embedded lyrics from the audio file.
/// Returns an empty lyricsList if no lyrics are available.
pub async fn get_lyrics_by_song_id(
    axum::extract::Query(params): axum::extract::Query<IdParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    use crate::scanner::lyrics::{parse_lrc, parse_unsynced};

    // Get the required 'id' parameter
    let Some(song_id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Get the song (also verifies it exists)
    let Some(song) = auth.state.get_song(song_id) else {
        return error_response(auth.format, &ApiError::NotFound("Song not found".into()))
            .into_response();
    };

    // Extract lyrics from the audio file
    let extracted = auth.state.get_song_lyrics(song_id);

    // Convert extracted lyrics to OpenSubsonic StructuredLyrics format
    let structured_lyrics: Vec<StructuredLyrics> = extracted
        .into_iter()
        .map(|lyrics| {
            let lang = lyrics.lang.unwrap_or_else(|| "und".to_string()); // "und" = undetermined

            if lyrics.synced {
                // Parse LRC format into timed lines
                let parsed = parse_lrc(&lyrics.text);
                let lines: Vec<LyricLine> = parsed
                    .into_iter()
                    .map(|l| LyricLine::synced(l.start_ms, l.text))
                    .collect();

                StructuredLyrics {
                    display_artist: song.artist_name.clone(),
                    display_title: Some(song.title.clone()),
                    lang,
                    offset: None,
                    synced: true,
                    lines,
                }
            } else {
                // Unsynced lyrics - split into lines
                let parsed = parse_unsynced(&lyrics.text);
                let lines: Vec<LyricLine> = parsed.into_iter().map(LyricLine::unsynced).collect();

                StructuredLyrics {
                    display_artist: song.artist_name.clone(),
                    display_title: Some(song.title.clone()),
                    lang,
                    offset: None,
                    synced: false,
                    lines,
                }
            }
        })
        .collect();

    let response = LyricsListResponse::new(structured_lyrics);

    ok_lyrics_list(auth.format, response).into_response()
}
