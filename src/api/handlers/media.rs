//! Media retrieval handlers (stream, download, cover art).
use axum::{
    body::Body,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use std::path::{Path, PathBuf};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;
use crate::api::handlers::repo_result_or_response;
use crate::api::response::error_response;
use crate::models::music::Song;
use crate::paths::resolve_cover_art_dir;

/// Validate that a song's path is within one of the configured music folders.
/// This prevents path traversal attacks where a malicious path in the database
/// could be used to read arbitrary files.
fn validate_song_path(song: &Song, auth: &SubsonicAuth) -> Result<PathBuf, &'static str> {
    let song_path = Path::new(&song.path);

    // Canonicalize the song path to resolve any symlinks and ../ components
    let Ok(canonical_path) = song_path.canonicalize() else {
        return Err("Audio file not found on disk");
    };

    // Get all music folders and verify the song is within one of them
    let music_folders = auth
        .state()
        .get_music_folders()
        .map_err(|_e| "Music folder lookup failed")?;
    for folder in &music_folders {
        if let Ok(folder_canonical) = Path::new(&folder.path).canonicalize()
            && canonical_path.starts_with(&folder_canonical)
        {
            return Ok(canonical_path);
        }
    }

    // Song path is not within any music folder - potential path traversal
    tracing::warn!(
        name = "media.path_validation.blocked",
        song.id = song.id,
        song.path = %song.path,
        "song path validation failed"
    );
    Err("Audio file not found in music library")
}

/// Query parameters for the stream endpoint.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct StreamParams {
    /// The ID of the song to stream.
    pub id: Option<String>,
    /// Maximum bit rate (currently ignored, no transcoding).
    #[serde(rename = "maxBitRate")]
    pub max_bit_rate: Option<i32>,
    /// Preferred format (currently ignored, no transcoding).
    pub format: Option<String>,
    /// Time offset in seconds (for video, currently ignored).
    #[serde(rename = "timeOffset")]
    pub time_offset: Option<i32>,
    /// Video size (for video, currently ignored).
    pub size: Option<String>,
    /// Whether to estimate content length (currently ignored).
    #[serde(rename = "estimateContentLength")]
    pub estimate_content_length: Option<bool>,
    /// Whether the client can handle transcoded content (currently ignored).
    pub converted: Option<bool>,
}

/// Stream a song file.
///
/// Returns the audio file as a binary stream. Supports HTTP range requests
/// for seeking within the file.
///
/// Parameters:
/// - `id` (required): The ID of the song to stream.
/// - `maxBitRate` (optional): Maximum bit rate for transcoding (not yet implemented).
/// - `format` (optional): Preferred format for transcoding (not yet implemented).
pub async fn stream(
    headers: HeaderMap,
    axum::extract::Query(params): axum::extract::Query<StreamParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Get song ID
    let Some(song_id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Look up song in database
    let song = match repo_result_or_response(auth.format, auth.state().get_song(song_id)) {
        Ok(Some(song)) => song,
        Ok(None) => {
            return error_response(auth.format, &ApiError::NotFound("Song not found".into()))
                .into_response();
        }
        Err(response) => return response,
    };

    // Check that user has stream permission
    if !auth.user.roles.stream_role {
        return error_response(auth.format, &ApiError::NotAuthorized).into_response();
    }

    // Validate the song path is within a music folder (prevents path traversal)
    let Ok(path) = validate_song_path(&song, &auth) else {
        return error_response(
            auth.format,
            &ApiError::NotFound("Audio file not found".into()),
        )
        .into_response();
    };

    // Open the file
    let Ok(file) = File::open(&path).await else {
        return error_response(
            auth.format,
            &ApiError::Generic("Failed to open audio file".into()),
        )
        .into_response();
    };

    // Get file metadata
    let Ok(metadata) = file.metadata().await else {
        return error_response(
            auth.format,
            &ApiError::Generic("Failed to read file metadata".into()),
        )
        .into_response();
    };

    let file_size = metadata.len();
    let content_type = song.content_type.clone();

    // Check for Range header to support seeking
    let range_header = headers.get(header::RANGE).and_then(|v| v.to_str().ok());

    if let Some(range) = range_header {
        // Parse range header (format: "bytes=start-end" or "bytes=start-")
        if let Some(range_spec) = range.strip_prefix("bytes=") {
            let parts: Vec<&str> = range_spec.split('-').collect();
            if parts.len() == 2 {
                let start: u64 = parts[0].parse().unwrap_or(0);
                let end: u64 = if parts[1].is_empty() {
                    file_size - 1
                } else {
                    parts[1].parse().unwrap_or(file_size - 1)
                };

                // Validate range
                if start >= file_size {
                    return (
                        StatusCode::RANGE_NOT_SATISFIABLE,
                        [(header::CONTENT_RANGE, format!("bytes */{file_size}"))],
                    )
                        .into_response();
                }

                let end = end.min(file_size - 1);
                let content_length = end - start + 1;

                // Seek to start position
                let mut file = file;
                if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
                    return error_response(
                        auth.format,
                        &ApiError::Generic("Failed to seek in file".into()),
                    )
                    .into_response();
                }

                // Create a limited reader for the range
                let stream = ReaderStream::new(file.take(content_length));
                let body = Body::from_stream(stream);

                return (
                    StatusCode::PARTIAL_CONTENT,
                    [
                        (header::CONTENT_TYPE, content_type),
                        (header::CONTENT_LENGTH, content_length.to_string()),
                        (
                            header::CONTENT_RANGE,
                            format!("bytes {start}-{end}/{file_size}"),
                        ),
                        (header::ACCEPT_RANGES, "bytes".to_string()),
                    ],
                    body,
                )
                    .into_response();
            }
        }
    }

    // No range requested, stream entire file
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_LENGTH, file_size.to_string()),
            (header::ACCEPT_RANGES, "bytes".to_string()),
        ],
        body,
    )
        .into_response()
}

/// Download a song file.
///
/// Similar to stream but with Content-Disposition header for downloading.
pub async fn download(
    axum::extract::Query(params): axum::extract::Query<StreamParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Get song ID
    let Some(song_id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Look up song in database
    let song = match repo_result_or_response(auth.format, auth.state().get_song(song_id)) {
        Ok(Some(song)) => song,
        Ok(None) => {
            return error_response(auth.format, &ApiError::NotFound("Song not found".into()))
                .into_response();
        }
        Err(response) => return response,
    };

    // Check that user has download permission
    if !auth.user.roles.download_role {
        return error_response(auth.format, &ApiError::NotAuthorized).into_response();
    }

    // Validate the song path is within a music folder (prevents path traversal)
    let Ok(path) = validate_song_path(&song, &auth) else {
        return error_response(
            auth.format,
            &ApiError::NotFound("Audio file not found".into()),
        )
        .into_response();
    };

    // Get filename for Content-Disposition and sanitize it to prevent header injection
    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download")
        .replace(['"', '\r', '\n'], "");

    // Open the file
    let Ok(file) = File::open(&path).await else {
        return error_response(
            auth.format,
            &ApiError::Generic("Failed to open audio file".into()),
        )
        .into_response();
    };

    // Get file metadata
    let Ok(metadata) = file.metadata().await else {
        return error_response(
            auth.format,
            &ApiError::Generic("Failed to read file metadata".into()),
        )
        .into_response();
    };

    let file_size = metadata.len();
    let content_type = song.content_type.clone();

    // Stream the file
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CONTENT_LENGTH, file_size.to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{filename}\""),
            ),
        ],
        body,
    )
        .into_response()
}

/// Query parameters for the getCoverArt endpoint.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct CoverArtParams {
    /// The ID of the cover art to retrieve (the hash stored in album/song `cover_art` field).
    pub id: Option<String>,
    /// Requested size (width/height in pixels). Currently ignored - returns original size.
    pub size: Option<u32>,
}

/// Get cover art for an album or song.
///
/// Returns the cover art image as binary data.
///
/// Parameters:
/// - `id` (required): The cover art ID (hash from the album/song coverArt field).
/// - `size` (optional): Requested size in pixels (not yet implemented).
pub async fn get_cover_art(
    axum::extract::Query(params): axum::extract::Query<CoverArtParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    // Get cover art ID
    let Some(cover_art_id) = params.id.as_ref().filter(|id| !id.is_empty()) else {
        return error_response(auth.format, &ApiError::MissingParameter("id".into()))
            .into_response();
    };

    // Check that user has coverArt permission
    if !auth.user.roles.cover_art_role {
        return error_response(auth.format, &ApiError::NotAuthorized).into_response();
    }

    // Get cover art cache directory
    let cover_art_dir = resolve_cover_art_dir();

    // Try to find the cover art file with different extensions
    let extensions = ["jpg", "jpeg", "png", "gif", "bmp", "tiff", "webp"];
    let mut cover_art_path = None;
    let mut content_type = "image/jpeg";

    for ext in &extensions {
        let path = cover_art_dir.join(format!("{cover_art_id}.{ext}"));
        if path.exists() {
            content_type = match *ext {
                "png" => "image/png",
                "gif" => "image/gif",
                "bmp" => "image/bmp",
                "tiff" => "image/tiff",
                "webp" => "image/webp",
                _ => "image/jpeg",
            };
            cover_art_path = Some(path);
            break;
        }
    }

    let Some(path) = cover_art_path else {
        return error_response(
            auth.format,
            &ApiError::NotFound("Cover art not found".into()),
        )
        .into_response();
    };

    // Open the file
    let Ok(file) = File::open(&path).await else {
        return error_response(
            auth.format,
            &ApiError::Generic("Failed to open cover art file".into()),
        )
        .into_response();
    };

    // Get file metadata
    let Ok(metadata) = file.metadata().await else {
        return error_response(
            auth.format,
            &ApiError::Generic("Failed to read file metadata".into()),
        )
        .into_response();
    };

    let file_size = metadata.len();

    // Stream the file
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::CONTENT_LENGTH, file_size.to_string()),
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable".to_string(),
            ), // Cache for 1 year (cover art is content-addressed)
        ],
        body,
    )
        .into_response()
}
