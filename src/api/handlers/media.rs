//! Media retrieval handlers (stream, download, cover art).
use axum::{
    body::Body,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use image::{GenericImageView, ImageFormat, imageops::FilterType};
use std::{
    io::Cursor,
    path::{Path, PathBuf},
};
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::io::ReaderStream;

use crate::api::auth::SubsonicContext;
use crate::api::handlers::util;
use crate::models::music::Song;
use crate::paths::{resolve_avatars_dir, resolve_cover_art_dir};

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

fn cover_art_image_format(extension: &str) -> Option<ImageFormat> {
    match extension {
        "jpg" | "jpeg" => Some(ImageFormat::Jpeg),
        "png" => Some(ImageFormat::Png),
        "gif" => Some(ImageFormat::Gif),
        "bmp" => Some(ImageFormat::Bmp),
        "tiff" => Some(ImageFormat::Tiff),
        "webp" => Some(ImageFormat::WebP),
        _ => None,
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

async fn read_cover_art_bytes(
    auth: &SubsonicContext,
    path: &Path,
    open_error: &'static str,
) -> Result<Vec<u8>, axum::response::Response> {
    tokio::fs::read(path).await.map_err(|error| {
        tracing::error!(path = %path.display(), error = %error, "failed to read cover art file");
        util::service_error(auth, open_error)
    })
}

async fn resize_cover_art_bytes(bytes: Vec<u8>, format: ImageFormat, size: u32) -> Option<Vec<u8>> {
    tokio::task::spawn_blocking(move || {
        let image = image::load_from_memory(&bytes).ok()?;
        let (width, height) = image.dimensions();
        if width <= size && height <= size {
            return Some(bytes);
        }

        let resized = image.resize(size, size, FilterType::Lanczos3);
        let mut output = Cursor::new(Vec::new());
        resized.write_to(&mut output, format).ok()?;
        Some(output.into_inner())
    })
    .await
    .ok()
    .flatten()
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
    let Some(song_id) = params
        .id
        .as_deref()
        .and_then(crate::models::music::EntityId::parse_song)
    else {
        return util::missing_param(&auth, "id");
    };

    // Look up song in database
    let song = match auth.music().get_song(song_id) {
        Ok(Some(song)) => song,
        Ok(None) => {
            return util::not_found(&auth, "Song not found");
        }
        Err(e) => {
            return util::repo_error(&auth, e);
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

/// Download a song file, or a zip archive for an album/artist/playlist id.
///
/// Similar to stream but with Content-Disposition header for downloading.
/// `al-`, `ar-`, and `pl-` prefixed ids are served as zip archives
/// (navidrome-compatible); bare and `mf-` ids serve the song directly.
pub async fn download(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<StreamParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    use crate::models::music::EntityId;

    let Some(id_str) = params.id.as_deref() else {
        return util::missing_param(&auth, "id");
    };
    let Some(entity_id) = EntityId::parse(id_str) else {
        return util::service_error(&auth, format!("Invalid id: {id_str}"));
    };

    // Check that user has download permission
    if !auth.user.roles.download_role {
        return util::unauthorized(&auth);
    }

    match entity_id {
        EntityId::Song(id) => match auth.music().get_song(id) {
            Ok(Some(song)) => download_song(&auth, &song).await,
            Ok(None) => util::not_found(&auth, "Song not found"),
            Err(e) => util::repo_error(&auth, e),
        },
        EntityId::Album(id) => match auth.music().get_album(id) {
            Ok(Some(album)) => {
                let songs = match auth.music().get_songs_by_album(id) {
                    Ok(songs) => songs,
                    Err(e) => return util::repo_error(&auth, e),
                };
                zip_download(&auth, &album.name, album_zip_entries(songs), None)
            }
            Ok(None) => util::not_found(&auth, "Album not found"),
            Err(e) => util::repo_error(&auth, e),
        },
        EntityId::Artist(id) => match auth.music().get_artist(id) {
            Ok(Some(artist)) => {
                let songs = match auth.music().get_songs_by_artist(id) {
                    Ok(songs) => songs,
                    Err(e) => return util::repo_error(&auth, e),
                };
                zip_download(&auth, &artist.name, artist_zip_entries(songs), None)
            }
            Ok(None) => util::not_found(&auth, "Artist not found"),
            Err(e) => util::repo_error(&auth, e),
        },
        EntityId::Playlist(id) => match auth.music().get_playlist(id) {
            Ok(Some(playlist)) => {
                let songs = match auth.music().get_playlist_songs(id) {
                    Ok(songs) => songs,
                    Err(e) => return util::repo_error(&auth, e),
                };
                let (entries, m3u) = playlist_zip_entries(songs, &playlist.name);
                zip_download(&auth, &playlist.name, entries, Some(m3u))
            }
            Ok(None) => util::not_found(&auth, "Playlist not found"),
            Err(e) => util::repo_error(&auth, e),
        },
    }
}

/// Whether the songs span more than one disc number.
fn is_multi_disc(songs: &[&Song]) -> bool {
    let mut discs: Vec<i32> = songs.iter().filter_map(|song| song.disc_number).collect();
    discs.sort_unstable();
    discs.dedup();
    discs.len() > 1
}

/// Build zip entries for an album download.
fn album_zip_entries(songs: Vec<Song>) -> Vec<(String, Song)> {
    let refs: Vec<&Song> = songs.iter().collect();
    let multi_disc = is_multi_disc(&refs);
    songs
        .into_iter()
        .map(|song| (album_zip_entry_name(&song, multi_disc), song))
        .collect()
}

/// Build zip entries for an artist download: albums sorted by name, with
/// per-album multi-disc detection.
fn artist_zip_entries(mut songs: Vec<Song>) -> Vec<(String, Song)> {
    use std::collections::HashMap;

    songs.sort_by_key(|song| {
        (
            song.album_name.clone().unwrap_or_default(),
            song.disc_number.unwrap_or(1),
            song.track_number.unwrap_or(0),
        )
    });

    let mut albums: HashMap<String, Vec<&Song>> = HashMap::new();
    for song in &songs {
        albums
            .entry(song.album_name.clone().unwrap_or_default())
            .or_default()
            .push(song);
    }
    let multi_disc_by_album: HashMap<String, bool> = albums
        .into_iter()
        .map(|(album, album_songs)| (album, is_multi_disc(&album_songs)))
        .collect();

    songs
        .into_iter()
        .map(|song| {
            let multi_disc = multi_disc_by_album
                .get(&song.album_name.clone().unwrap_or_default())
                .copied()
                .unwrap_or(false);
            (album_zip_entry_name(&song, multi_disc), song)
        })
        .collect()
}

/// Build zip entries and an M3U index for a playlist download.
fn playlist_zip_entries(
    songs: Vec<Song>,
    playlist_name: &str,
) -> (Vec<(String, Song)>, (String, String)) {
    use std::fmt::Write as _;

    let mut m3u = format!("#EXTM3U\n#PLAYLIST:{playlist_name}\n");
    let entries: Vec<(String, Song)> = songs
        .into_iter()
        .enumerate()
        .map(|(index, song)| {
            let entry_name = playlist_zip_entry_name(&song, index);
            let _ = write!(
                m3u,
                "#EXTINF:{},{artist} - {title}\n{entry_name}\n",
                song.duration,
                artist = song.artist_name.as_deref().unwrap_or("Unknown Artist"),
                title = song.title,
            );
            (entry_name, song)
        })
        .collect();
    let m3u_name = format!("{}.m3u", sanitize_zip_component(playlist_name));
    (entries, (m3u_name, m3u))
}

/// Serve a single song file as a download attachment.
async fn download_song(auth: &SubsonicContext, song: &Song) -> axum::response::Response {
    // Validate the song path is within a music folder (prevents path traversal)
    let Ok(path) = validate_song_path(song, auth) else {
        return util::not_found(auth, "Audio file not found");
    };

    // Get filename for Content-Disposition and sanitize it to prevent header injection
    let filename = sanitized_filename(&path);

    let (file, file_size) =
        match open_file_with_size(auth, &path, "Failed to open audio file").await {
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

/// Sanitize a path component for use inside a zip archive.
fn sanitize_zip_component(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect()
}

/// Zip entry path for a song in an album archive, navidrome-style:
/// `<Album>/<filename>`, with a `Disc NN/` subfolder for multi-disc albums.
fn album_zip_entry_name(song: &Song, multi_disc: bool) -> String {
    let file = sanitized_filename(Path::new(&song.path));
    let album_dir = sanitize_zip_component(song.album_name.as_deref().unwrap_or("Unknown Album"));
    if multi_disc {
        format!(
            "{album_dir}/Disc {:02}/{file}",
            song.disc_number.unwrap_or(1)
        )
    } else {
        format!("{album_dir}/{file}")
    }
}

/// Zip entry path for a song in a playlist archive, navidrome-style:
/// `NN - Artist - Title.ext`.
fn playlist_zip_entry_name(song: &Song, index: usize) -> String {
    let artist = sanitize_zip_component(song.artist_name.as_deref().unwrap_or("Unknown Artist"));
    let title = sanitize_zip_component(&song.title);
    format!("{:02} - {artist} - {title}.{}", index + 1, song.suffix)
}

/// Stream a zip archive of the given songs as a download attachment.
///
/// The archive is written on a blocking thread into a bounded duplex stream
/// so large collections don't have to be buffered in memory.
fn zip_download(
    auth: &SubsonicContext,
    name: &str,
    entries: Vec<(String, Song)>,
    m3u: Option<(String, String)>,
) -> axum::response::Response {
    // Validate all paths up front; entries that fail are skipped
    let entries: Vec<(String, std::path::PathBuf)> = entries
        .into_iter()
        .filter_map(|(entry_name, song)| {
            validate_song_path(&song, auth)
                .ok()
                .map(|path| (entry_name, path))
        })
        .collect();
    if entries.is_empty() {
        return util::not_found(auth, "Audio files not found");
    }

    let (writer, reader) = tokio::io::duplex(256 * 1024);
    tokio::task::spawn_blocking(move || {
        use std::io::Write as _;

        // Streaming writer: no Seek needed, data descriptors emitted inline
        let mut zip = zip::ZipWriter::new_stream(tokio_util::io::SyncIoBridge::new(writer));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        for (entry_name, path) in entries {
            let Ok(mut file) = std::fs::File::open(&path) else {
                continue;
            };
            if zip.start_file(entry_name, options).is_err() {
                break;
            }
            if std::io::copy(&mut file, &mut zip).is_err() {
                break;
            }
        }

        if let Some((m3u_name, m3u_content)) = m3u
            && zip.start_file(m3u_name, options).is_ok()
        {
            let _ = zip.write_all(m3u_content.as_bytes());
        }

        let _ = zip.finish();
    });

    // navidrome replaces commas in the attachment filename
    let attachment_name = sanitize_zip_component(&name.replace(',', "_"));
    let body = Body::from_stream(ReaderStream::new(reader));

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/zip".to_string()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{attachment_name}.zip\""),
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
    /// Requested size (width/height in pixels). Images are scaled to fit within this square without upscaling.
    pub size: Option<u32>,
}

/// Get cover art for an album or song.
///
/// Returns the cover art image as binary data.
///
/// Parameters:
/// - `id` (required): The cover art ID (hash from the album/song coverArt field).
/// - `size` (optional): Maximum width/height in pixels. Images are scaled to fit within that square without upscaling.
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

    // Zero is not meaningful here; treat it as if the client omitted size.
    let requested_size = params.size.filter(|size| *size > 0);

    // Get cover art cache directory
    let cover_art_dir = resolve_cover_art_dir();

    // Find the cover art file (content-addressed `<hash>.<ext>` convention)
    let Some((path, cover_art_extension)) =
        crate::cover_art::find_file(&cover_art_dir, cover_art_id)
    else {
        return util::not_found(&auth, "Cover art not found");
    };
    let content_type = crate::cover_art::mime_from_extension(cover_art_extension);

    if let Some(size) = requested_size {
        let original_bytes =
            match read_cover_art_bytes(&auth, &path, "Failed to open cover art file").await {
                Ok(bytes) => bytes,
                Err(response) => return response,
            };

        let bytes = if let Some(image_format) = cover_art_image_format(cover_art_extension) {
            resize_cover_art_bytes(original_bytes.clone(), image_format, size)
                .await
                .unwrap_or_else(|| {
                    tracing::warn!(
                        cover_art_id = %cover_art_id,
                        requested_size = size,
                        path = %path.display(),
                        "failed to resize cover art; serving original"
                    );
                    original_bytes
                })
        } else {
            original_bytes
        };

        return (
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, content_type.to_string()),
                (header::CONTENT_LENGTH, bytes.len().to_string()),
                (
                    header::CACHE_CONTROL,
                    "public, max-age=31536000, immutable".to_string(),
                ), // Cache for 1 year (cover art is content-addressed)
            ],
            Body::from(bytes),
        )
            .into_response();
    }

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

/// Query parameters for the getAvatar endpoint.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct AvatarParams {
    /// The user whose avatar to retrieve.
    pub username: Option<String>,
}

/// Supported avatar file extensions, in lookup order.
const AVATAR_EXTENSIONS: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

/// Get a user's avatar image.
///
/// Avatars are served from the `avatars` directory under the data root,
/// named `<username>.<ext>` (png, jpg, jpeg, or webp). Returns a Subsonic
/// not-found error when the user has no avatar file.
pub async fn get_avatar(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<AvatarParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(username) = params.username.as_deref().filter(|name| !name.is_empty()) else {
        return util::missing_param(&auth, "username");
    };

    // Reject anything that isn't a plain username (prevents path traversal)
    if !username
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    {
        return util::not_found(&auth, "Avatar not found");
    }

    let avatars_dir = resolve_avatars_dir();
    let avatar = AVATAR_EXTENSIONS
        .iter()
        .map(|ext| (avatars_dir.join(format!("{username}.{ext}")), *ext))
        .find(|(path, _)| path.is_file());

    let Some((path, ext)) = avatar else {
        return util::not_found(&auth, "Avatar not found");
    };

    let content_type = match ext {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    };

    let (file, file_size) = match open_file_with_size(&auth, &path, "Failed to open avatar").await {
        Ok(file) => file,
        Err(response) => return response,
    };

    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);

    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type.to_string()),
            (header::CONTENT_LENGTH, file_size.to_string()),
            (header::CACHE_CONTROL, "private, max-age=3600".to_string()),
        ],
        body,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, path::Path};

    use image::{DynamicImage, GenericImageView, ImageBuffer, ImageFormat, Rgba};

    use super::{
        album_zip_entry_name, cover_art_image_format, is_safe_cover_art_id, parse_byte_range,
        playlist_zip_entry_name, resize_cover_art_bytes, sanitize_zip_component,
        sanitized_filename,
    };
    use crate::models::music::Song;

    fn test_cover_art_bytes(width: u32, height: u32) -> Vec<u8> {
        let image = DynamicImage::ImageRgba8(ImageBuffer::from_fn(width, height, |x, y| {
            Rgba([(x % 255) as u8, (y % 255) as u8, 128, 255])
        }));

        let mut bytes = Cursor::new(Vec::new());
        image
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("test image should encode");
        bytes.into_inner()
    }

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
    fn cover_art_image_format_matches_supported_extensions() {
        assert_eq!(cover_art_image_format("jpg"), Some(ImageFormat::Jpeg));
        assert_eq!(cover_art_image_format("jpeg"), Some(ImageFormat::Jpeg));
        assert_eq!(cover_art_image_format("png"), Some(ImageFormat::Png));
        assert_eq!(cover_art_image_format("gif"), Some(ImageFormat::Gif));
        assert_eq!(cover_art_image_format("bmp"), Some(ImageFormat::Bmp));
        assert_eq!(cover_art_image_format("tiff"), Some(ImageFormat::Tiff));
        assert_eq!(cover_art_image_format("webp"), Some(ImageFormat::WebP));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cover_art_resize_keeps_original_bytes_when_image_is_small() {
        let original = test_cover_art_bytes(2, 2);
        let resized = resize_cover_art_bytes(original.clone(), ImageFormat::Png, 8)
            .await
            .expect("small image should be returned unchanged");
        assert_eq!(resized, original);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cover_art_resize_downscales_large_images_without_upscaling() {
        let original = test_cover_art_bytes(8, 4);
        let resized = resize_cover_art_bytes(original.clone(), ImageFormat::Png, 4)
            .await
            .expect("large image should resize");

        assert_ne!(resized, original);

        let decoded = image::load_from_memory(&resized).expect("resized image should decode");
        let (width, height) = decoded.dimensions();
        assert!(width <= 4);
        assert!(height <= 4);
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

    fn test_song() -> Song {
        Song {
            id: 1,
            title: "Track: One?".to_string(),
            sort_name: None,
            album_id: Some(1),
            artist_id: Some(1),
            artist_name: Some("The/Artist".to_string()),
            album_name: Some("My <Album>".to_string()),
            music_folder_id: 1,
            path: "/music/Album/01 - Track One.flac".to_string(),
            parent_path: "/music/Album".to_string(),
            file_size: 100,
            content_type: "audio/flac".to_string(),
            suffix: "flac".to_string(),
            duration: 60,
            bit_rate: None,
            bit_depth: None,
            sampling_rate: None,
            channel_count: None,
            track_number: Some(1),
            disc_number: Some(1),
            year: None,
            genre: None,
            cover_art: None,
            musicbrainz_id: None,
            play_count: 0,
            created_at: chrono::NaiveDate::from_ymd_opt(2024, 1, 1)
                .expect("valid date")
                .and_hms_opt(0, 0, 0)
                .expect("valid time"),
            updated_at: chrono::NaiveDate::from_ymd_opt(2024, 1, 1)
                .expect("valid date")
                .and_hms_opt(0, 0, 0)
                .expect("valid time"),
        }
    }

    #[test]
    fn sanitize_zip_component_replaces_illegal_characters() {
        assert_eq!(
            sanitize_zip_component("a/b\\c:d*e?f\"g<h>i|j"),
            "a_b_c_d_e_f_g_h_i_j"
        );
        assert_eq!(sanitize_zip_component("plain name"), "plain name");
    }

    #[test]
    fn album_zip_entry_uses_album_dir_and_original_filename() {
        let song = test_song();
        assert_eq!(
            album_zip_entry_name(&song, false),
            "My _Album_/01 - Track One.flac"
        );
        assert_eq!(
            album_zip_entry_name(&song, true),
            "My _Album_/Disc 01/01 - Track One.flac"
        );
    }

    #[test]
    fn playlist_zip_entry_uses_position_artist_title() {
        let song = test_song();
        assert_eq!(
            playlist_zip_entry_name(&song, 0),
            "01 - The_Artist - Track_ One_.flac"
        );
    }

    #[test]
    fn streaming_zip_writer_produces_readable_archive() {
        // The download path uses ZipWriter::new_stream over a non-seekable
        // writer; verify the produced bytes parse back with entry names.
        let mut sink = std::io::Cursor::new(Vec::new());
        {
            use std::io::Write as _;
            let mut zip = zip::ZipWriter::new_stream(&mut sink);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip.start_file("Album/01 - One.flac", options)
                .expect("start file");
            zip.write_all(b"data1").expect("write");
            zip.start_file("Album/02 - Two.flac", options)
                .expect("start file");
            zip.write_all(b"data2").expect("write");
            zip.finish().expect("finish");
        }

        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(sink.into_inner()))
            .expect("archive must parse");
        assert_eq!(archive.len(), 2);
        assert_eq!(
            archive.by_index(0).expect("entry").name(),
            "Album/01 - One.flac"
        );
        assert_eq!(
            archive.by_index(1).expect("entry").name(),
            "Album/02 - Two.flac"
        );
    }
}
