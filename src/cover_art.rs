//! Cover art storage conventions.
//!
//! Cover art is stored in the cover art directory as content-addressed files
//! named `<md5-hash>.<ext>` (e.g. `a1b2c3....jpg`). The MD5 hash of the image
//! bytes is used as the cover art ID, so identical images deduplicate and the
//! filename is immutable once written.
//!
//! This module owns the naming convention and the image extension/MIME
//! mapping, so both the scanner (which writes cover art) and the media
//! handlers (which read it) stay in sync.

use std::path::{Path, PathBuf};

/// Supported image extensions for cover art files, in lookup order.
pub const COVER_ART_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "tiff", "webp"];

/// Map a stored cover art file extension to its MIME type.
#[must_use]
pub fn mime_from_extension(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "tiff" => "image/tiff",
        "webp" => "image/webp",
        _ => "image/jpeg",
    }
}

/// Map an image MIME type to the file extension used for stored cover art.
#[must_use]
pub fn extension_from_mime(mime: &str) -> &'static str {
    match mime {
        "image/webp" => "webp",
        "image/png" => "png",
        "image/gif" => "gif",
        "image/bmp" => "bmp",
        "image/tiff" => "tiff",
        _ => "jpg", // Default to JPEG
    }
}

/// Build the cover art filename for a content-addressed ID and extension.
#[must_use]
pub fn filename(id: &str, extension: &str) -> String {
    format!("{id}.{extension}")
}

/// Find an existing cover art file for the given ID in the given directory,
/// returning its path and extension. Tries every supported extension.
#[must_use]
pub fn find_file(dir: &Path, id: &str) -> Option<(PathBuf, &'static str)> {
    COVER_ART_EXTENSIONS.iter().find_map(|extension| {
        let path = dir.join(filename(id, extension));
        path.is_file().then_some((path, *extension))
    })
}

/// Compute the MD5 content hash used as a cover art ID.
#[must_use]
pub fn content_hash(data: &[u8]) -> String {
    use md5::{Digest, Md5};
    let mut hasher = Md5::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::{COVER_ART_EXTENSIONS, content_hash, extension_from_mime, mime_from_extension};

    #[test]
    fn cover_art_extensions_cover_all_handled_mimes() {
        assert!(COVER_ART_EXTENSIONS.contains(&"jpg"));
        assert!(COVER_ART_EXTENSIONS.contains(&"png"));
        assert!(COVER_ART_EXTENSIONS.contains(&"gif"));
        assert!(COVER_ART_EXTENSIONS.contains(&"bmp"));
        assert!(COVER_ART_EXTENSIONS.contains(&"tiff"));
        assert!(COVER_ART_EXTENSIONS.contains(&"webp"));
    }

    #[test]
    fn mime_extension_roundtrip_is_stable_for_known_formats() {
        for extension in COVER_ART_EXTENSIONS {
            let mime = mime_from_extension(extension);
            let roundtrip = extension_from_mime(mime);
            // jpeg is stored as jpg.
            let canonical = if *extension == "jpeg" {
                "jpg"
            } else {
                *extension
            };
            assert_eq!(roundtrip, canonical, "{extension} -> {mime}");
        }
    }

    #[test]
    fn unknown_mime_defaults_to_jpeg() {
        assert_eq!(extension_from_mime("image/avif"), "jpg");
        assert_eq!(mime_from_extension("avif"), "image/jpeg");
    }

    #[test]
    fn content_hash_is_stable_and_content_addressed() {
        assert_eq!(content_hash(b"abc"), content_hash(b"abc"));
        assert_ne!(content_hash(b"abc"), content_hash(b"abd"));
    }
}
