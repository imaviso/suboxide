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

use crate::api::auth::SubsonicContext;
use crate::api::handlers::util;
use crate::models::music::Song;
use crate::paths::resolve_cover_art_dir;

/// Validate that a song's path is within one of the configured music folders.
/// This prevents path traversal attacks where a malicious path in the database
/// could be used to read arbitrary files.
fn validate_song_path(song: &Song, auth: &SubsonicContext) -> Result<PathBuf, &'static str> {
    let song_path = Path::new(&song.path);

    // Canonicalize the song path to resolve any symlinks and ../ components
    let Ok(canonical_path) = song_path.canonicalize() else {
        return Err("Audio file not found on disk");
    };

    // Get all music folders and verify the song is within one of them
    let music_folders = auth
        .music()
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

fn is_safe_cover_art_id(id: &str) -> bool {
    Path::new(id).file_name().and_then(|name| name.to_str()) == Some(id)
        && !id.contains("..")
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn sanitized_filename(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download")
        .replace(['"', '\r', '\n'], "")
}

fn cover_art_content_type(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tiff" => "image/tiff",
        "webp" => "image/webp",
        _ => "image/jpeg",
    }
}

struct ByteRange {
    start: u64,
    end: u64,
}

impl ByteRange {
    const fn len(&self) -> u64 {
        self.end - self.start + 1
    }
}

fn parse_byte_range(range: &str, file_size: u64) -> Option<ByteRange> {
    let range_spec = range.strip_prefix("bytes=")?;
    let (start, end) = range_spec.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    if start >= file_size {
        return None;
    }
    let end = if end.is_empty() {
        file_size.saturating_sub(1)
    } else {
        end.parse::<u64>().ok()?.min(file_size - 1)
    };
    if end < start {
        return None;
    }

    Some(ByteRange { start, end })
}

async fn open_file_with_size(
    auth: &SubsonicContext,
    path: &Path,
    open_error: &'static str,
) -> Result<(File, u64), axum::response::Response> {
    let file = File::open(path).await.map_err(|error| {
        tracing::error!(path = %path.display(), error = %error, "failed to open media file");
        util::service_error(auth, open_error)
    })?;

    let metadata = file.metadata().await.map_err(|error| {
        tracing::error!(path = %path.display(), error = %error, "failed to read media metadata");
        util::service_error(auth, "Failed to read file metadata")
    })?;

    Ok((file, metadata.len()))
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<StreamParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    // Get song ID
    let Some(song_id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return util::missing_param(&auth, "id");
    };

    // Look up song in database
    let song = match auth.music().get_song(song_id) {
        Ok(Some(song)) => song,
        Ok(None) => {
            return util::not_found(&auth, "Song not found");
        }
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    // Check that user has stream permission
    if !auth.user.roles.stream_role {
        return util::unauthorized(&auth);
    }

    // Validate the song path is within a music folder (prevents path traversal)
    let Ok(path) = validate_song_path(&song, &auth) else {
        return util::not_found(&auth, "Audio file not found");
    };

    let (file, file_size) =
        match open_file_with_size(&auth, &path, "Failed to open audio file").await {
            Ok(file) => file,
            Err(response) => return response,
        };
    let content_type = song.content_type.clone();

    // Check for Range header to support seeking
    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok())
        && range.starts_with("bytes=")
    {
        if let Some(byte_range) = parse_byte_range(range, file_size) {
            let content_length = byte_range.len();

            let mut file = file;
            if let Err(e) = file.seek(std::io::SeekFrom::Start(byte_range.start)).await {
                tracing::error!(error = %e, "Failed to seek in file");
                return util::service_error(&auth, "Failed to seek in file");
            }

            let stream = ReaderStream::new(file.take(content_length));
            let body = Body::from_stream(stream);

            return (
                StatusCode::PARTIAL_CONTENT,
                [
                    (header::CONTENT_TYPE, content_type),
                    (header::CONTENT_LENGTH, content_length.to_string()),
                    (
                        header::CONTENT_RANGE,
                        format!(
                            "bytes {}-{}/{}",
                            byte_range.start, byte_range.end, file_size
                        ),
                    ),
                    (header::ACCEPT_RANGES, "bytes".to_string()),
                ],
                body,
            )
                .into_response();
        }

        if range.strip_prefix("bytes=").is_some_and(|range_spec| {
            range_spec
                .split_once('-')
                .and_then(|(start, _)| start.parse::<u64>().ok())
                .is_some_and(|start| start >= file_size)
        }) {
            return (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CONTENT_RANGE, format!("bytes */{file_size}"))],
            )
                .into_response();
        }

        return util::service_error(&auth, "Invalid byte range");
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<StreamParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    // Get song ID
    let Some(song_id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return util::missing_param(&auth, "id");
    };

    // Look up song in database
    let song = match auth.music().get_song(song_id) {
        Ok(Some(song)) => song,
        Ok(None) => {
            return util::not_found(&auth, "Song not found");
        }
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    // Check that user has download permission
    if !auth.user.roles.download_role {
        return util::unauthorized(&auth);
    }

    // Validate the song path is within a music folder (prevents path traversal)
    let Ok(path) = validate_song_path(&song, &auth) else {
        return util::not_found(&auth, "Audio file not found");
    };

    // Get filename for Content-Disposition and sanitize it to prevent header injection
    let filename = sanitized_filename(&path);

    let (file, file_size) =
        match open_file_with_size(&auth, &path, "Failed to open audio file").await {
            Ok(file) => file,
            Err(response) => return response,
        };
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<CoverArtParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    // Get cover art ID
    let Some(cover_art_id) = params.id.as_ref().filter(|id| !id.is_empty()) else {
        return util::missing_param(&auth, "id");
    };
    if !is_safe_cover_art_id(cover_art_id) {
        return util::not_found(&auth, "Cover art");
    }

    // Check that user has coverArt permission
    if !auth.user.roles.cover_art_role {
        return util::unauthorized(&auth);
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
            content_type = cover_art_content_type(ext);
            cover_art_path = Some(path);
            break;
        }
    }

    let Some(path) = cover_art_path else {
        return util::not_found(&auth, "Cover art not found");
    };

    let (file, file_size) =
        match open_file_with_size(&auth, &path, "Failed to open cover art file").await {
            Ok(file) => file,
            Err(response) => return response,
        };

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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{
        cover_art_content_type, is_safe_cover_art_id, parse_byte_range, sanitized_filename,
    };

    #[test]
    fn safe_cover_art_id_allows_content_hash_like_values() {
        assert!(is_safe_cover_art_id("abc123"));
        assert!(is_safe_cover_art_id("cover-art_2024"));
    }

    #[test]
    fn safe_cover_art_id_rejects_paths_traversal_dots_and_extensions() {
        assert!(!is_safe_cover_art_id("../secret"));
        assert!(!is_safe_cover_art_id("nested/cover"));
        assert!(!is_safe_cover_art_id("cover.jpg"));
        assert!(!is_safe_cover_art_id(""));
    }

    #[test]
    fn sanitized_filename_removes_content_disposition_breakout_characters() {
        assert_eq!(
            sanitized_filename(Path::new("/music/evil\"\r\nname.flac")),
            "evilname.flac"
        );
        assert_eq!(sanitized_filename(Path::new("/music")), "music");
    }

    #[test]
    fn cover_art_content_type_matches_supported_extensions() {
        assert_eq!(cover_art_content_type("jpg"), "image/jpeg");
        assert_eq!(cover_art_content_type("jpeg"), "image/jpeg");
        assert_eq!(cover_art_content_type("png"), "image/png");
        assert_eq!(cover_art_content_type("gif"), "image/gif");
        assert_eq!(cover_art_content_type("bmp"), "image/bmp");
        assert_eq!(cover_art_content_type("tiff"), "image/tiff");
        assert_eq!(cover_art_content_type("webp"), "image/webp");
    }

    #[test]
    fn byte_range_parser_accepts_open_and_closed_ranges() {
        let closed = parse_byte_range("bytes=2-5", 10).expect("closed range should parse");
        assert_eq!((closed.start, closed.end, closed.len()), (2, 5, 4));

        let open = parse_byte_range("bytes=4-", 10).expect("open range should parse");
        assert_eq!((open.start, open.end, open.len()), (4, 9, 6));
    }

    #[test]
    fn byte_range_parser_rejects_invalid_or_unsatisfiable_ranges() {
        assert!(parse_byte_range("items=1-2", 10).is_none());
        assert!(parse_byte_range("bytes=bogus-2", 10).is_none());
        assert!(parse_byte_range("bytes=5-2", 10).is_none());
        assert!(parse_byte_range("bytes=10-12", 10).is_none());
    }
}
