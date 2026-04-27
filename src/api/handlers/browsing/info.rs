//! Info retrieval handlers (artist info, album info, lyrics).

use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;
use crate::api::handlers::browsing::IdParams;
use crate::api::response::{SubsonicResponse, error_response};
use crate::models::music::{
    AlbumInfoResponse, LyricLine, LyricsListResponse, LyricsResponse, StructuredLyrics,
};

/// Query parameters for getArtistInfo2.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ArtistInfo2Params {
    /// The artist ID.
    pub id: Option<i32>,
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
    let Some(artist_id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Get the artist
    let response = auth.state.get_artist_info_with_cache(artist_id);
    SubsonicResponse::artist_info2(auth.format, response).into_response()
}

/// Query parameters for getAlbumInfo2.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AlbumInfo2Params {
    /// The album ID.
    pub id: Option<i32>,
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
    let Some(album_id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Get the album
    let Some(album) = auth.state.get_album(album_id) else {
        return error_response(auth.format, &ApiError::NotFound("Album".into())).into_response();
    };

    // Create response with available data from the album
    let response = AlbumInfoResponse::from_album(&album);
    SubsonicResponse::album_info(auth.format, response).into_response()
}

/// GET/POST /rest/getArtistInfo[.view]
///
/// Returns artist info (non-ID3 version). Similar to getArtistInfo2.
pub async fn get_artist_info(
    axum::extract::Query(params): axum::extract::Query<ArtistInfo2Params>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Get the required 'id' parameter
    let Some(artist_id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Get the artist info with cache
    let response = auth.state.get_artist_info_non_id3_with_cache(artist_id);
    SubsonicResponse::artist_info(auth.format, response).into_response()
}

/// GET/POST /rest/getAlbumInfo[.view]
///
/// Returns album info (non-ID3 version). Similar to getAlbumInfo2.
pub async fn get_album_info(
    axum::extract::Query(params): axum::extract::Query<AlbumInfo2Params>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Get the required 'id' parameter
    let Some(album_id) = params.id else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Get the album
    let Some(album) = auth.state.get_album(album_id) else {
        return error_response(auth.format, &ApiError::NotFound("Album".into())).into_response();
    };

    // Use AlbumInfoResponse which is the same for ID3 and non-ID3
    let response = AlbumInfoResponse::from_album(&album);
    SubsonicResponse::album_info(auth.format, response).into_response()
}

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
pub async fn get_lyrics(
    axum::extract::Query(params): axum::extract::Query<LyricsParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let artist = params.artist.as_deref().unwrap_or_default();
    let title = params.title.as_deref().unwrap_or_default();

    // Try to find a song with the requested artist and title
    let mut lyrics_content = None;
    if !artist.is_empty()
        && !title.is_empty()
        && let Some(song) = auth.state.find_song_by_artist_and_title(artist, title)
    {
        // Found a song, extract its lyrics
        let extracted = auth.state.get_song_lyrics(song.id);
        // Just take the first one and use its text
        if let Some(lyrics) = extracted.first() {
            lyrics_content = Some(lyrics.text.clone());
        }
    }

    let response = LyricsResponse::new(params.artist.clone(), params.title.clone(), lyrics_content);

    SubsonicResponse::lyrics(auth.format, response)
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
    let Some(song_id) = params.id else {
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

    SubsonicResponse::lyrics_list(auth.format, response).into_response()
}
