//! Music library scanner.
//!
//! Walks music folders, reads audio file metadata, and populates the database.
//! Supports incremental scanning (only changed files) and auto-scan with configurable interval.

pub mod lyrics;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, UNIX_EPOCH};

use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::{Accessor, ItemKey};
use rayon::prelude::*;
use thiserror::Error;
use tokio::sync::watch;
use walkdir::WalkDir;

use crate::db::{DbPool, MusicFolderRepository, MusicRepoError};
use crate::models::music::MusicFolder;

/// Errors that can occur during scanning.
#[derive(Debug, Error)]
pub enum ScanError {
    #[error("Database error: {0}")]
    Database(#[from] MusicRepoError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("No music folders configured")]
    NoMusicFolders,

    #[error("Music folder not found: {0}")]
    FolderNotFound(String),
}

/// Supported audio file extensions.
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "opus", "m4a", "aac", "wav", "wma", "aiff", "ape", "wv",
];

/// Common cover art filenames to look for in album directories.
/// These are tried in order of preference.
const COVER_ART_FILENAMES: &[&str] = &[
    "cover",
    "folder",
    "front",
    "album",
    "albumart",
    "albumartsmall",
    "thumb",
    "art",
];

/// Supported image file extensions for external cover art.
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "gif", "bmp", "webp"];

/// Batch size for song inserts (`SQLite` has a limit of ~999 variables per query)
const BATCH_SIZE: usize = 100;

/// Metadata extracted from an audio file.
#[derive(Debug, Clone)]
pub struct ScannedTrack {
    pub path: PathBuf,
    pub parent_path: String,
    pub file_size: u64,
    pub content_type: String,
    pub suffix: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track_number: Option<u32>,
    pub disc_number: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub duration_secs: u32,
    pub bit_rate: Option<u32>,
    pub bit_depth: Option<u8>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    /// Embedded cover art data (bytes).
    pub cover_art_data: Option<Vec<u8>>,
    /// MIME type of the embedded cover art.
    pub cover_art_mime: Option<String>,
    /// File modification time (Unix timestamp in seconds).
    pub file_modified_at: Option<i64>,
}

#[derive(Debug)]
struct PreparedTrack {
    track: ScannedTrack,
    path_str: String,
    artist_id: Option<i32>,
    album_id: Option<i32>,
    cover_art: Option<String>,
    is_update: bool,
}

/// Result of scanning a music folder.
#[derive(Debug, Default)]
pub struct ScanResult {
    pub tracks_found: usize,
    pub tracks_added: usize,
    pub tracks_updated: usize,
    pub tracks_skipped: usize,
    pub tracks_removed: usize,
    pub tracks_failed: usize,
    pub artists_added: usize,
    pub albums_added: usize,
    pub cover_art_saved: usize,
}

/// Shared state for tracking scan progress across API requests.
///
/// This is designed to be shared across threads (wrapped in Arc) and
/// provides atomic operations for checking and updating scan status.
#[derive(Debug, Default)]
pub struct ScanState {
    /// Whether a scan is currently in progress.
    scanning: AtomicBool,
    /// Number of items scanned so far.
    count: AtomicU64,
    /// Total number of items to scan (0 if unknown/discovery phase).
    total: AtomicU64,
    /// Current scan phase.
    phase: std::sync::RwLock<ScanPhase>,
    /// Current folder being scanned (if any).
    current_folder: std::sync::RwLock<Option<String>>,
}

/// A handle to a shared scan state.
///
/// This is a cheap-to-clone handle that provides access to the scan state
/// without exposing the internal `Arc`.
#[derive(Debug, Clone)]
pub struct ScanStateHandle(Arc<ScanState>);

impl ScanStateHandle {
    /// Create a new handle wrapping the given scan state.
    #[must_use]
    pub fn new(state: ScanState) -> Self {
        Self(Arc::new(state))
    }

    /// Get a reference to the underlying scan state.
    #[must_use]
    pub fn get(&self) -> &ScanState {
        &self.0
    }
}

impl std::ops::Deref for ScanStateHandle {
    type Target = ScanState;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Scan phase for progress tracking.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum ScanPhase {
    /// Not scanning.
    #[default]
    Idle,
    /// Discovering files on disk.
    Discovering,
    /// Processing/importing tracks.
    Processing,
    /// Cleaning up orphaned records.
    Cleaning,
}

impl ScanPhase {
    /// Get the phase as a string for API responses.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Discovering => "discovering",
            Self::Processing => "processing",
            Self::Cleaning => "cleaning",
        }
    }
}

impl ScanState {
    /// Create a new scan state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            scanning: AtomicBool::new(false),
            count: AtomicU64::new(0),
            total: AtomicU64::new(0),
            phase: std::sync::RwLock::new(ScanPhase::Idle),
            current_folder: std::sync::RwLock::new(None),
        }
    }

    /// Check if a scan is currently in progress.
    pub fn is_scanning(&self) -> bool {
        self.scanning.load(Ordering::SeqCst)
    }

    /// Get the current item count.
    pub fn get_count(&self) -> u64 {
        self.count.load(Ordering::SeqCst)
    }

    /// Get the total item count (0 if unknown).
    pub fn get_total(&self) -> u64 {
        self.total.load(Ordering::SeqCst)
    }

    /// Get the current scan phase.
    pub fn get_phase(&self) -> ScanPhase {
        self.phase
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Get the current folder being scanned.
    pub fn get_current_folder(&self) -> Option<String> {
        self.current_folder
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Try to start a scan. Returns false if a scan is already in progress.
    pub fn try_start(&self) -> bool {
        self.scanning
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Mark the scan as complete.
    pub fn finish(&self) {
        self.scanning.store(false, Ordering::SeqCst);
        *self
            .phase
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ScanPhase::Idle;
        *self
            .current_folder
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    /// Reset the count to 0.
    pub fn reset_count(&self) {
        self.count.store(0, Ordering::SeqCst);
    }

    /// Reset all progress state for a new scan.
    pub fn reset(&self) {
        self.count.store(0, Ordering::SeqCst);
        self.total.store(0, Ordering::SeqCst);
        *self
            .phase
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = ScanPhase::Idle;
        *self
            .current_folder
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
    }

    /// Increment the count by 1 and return the new value.
    pub fn increment_count(&self) -> u64 {
        self.count.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Set the count to a specific value.
    pub fn set_count(&self, value: u64) {
        self.count.store(value, Ordering::SeqCst);
    }

    /// Set the total item count.
    pub fn set_total(&self, value: u64) {
        self.total.store(value, Ordering::SeqCst);
    }

    /// Set the current scan phase.
    pub fn set_phase(&self, phase: ScanPhase) {
        *self
            .phase
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = phase;
    }

    /// Set the current folder being scanned.
    pub fn set_current_folder(&self, folder: Option<String>) {
        *self
            .current_folder
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = folder;
    }
}

/// Default cover art cache directory.
const COVER_ART_CACHE_DIR: &str = ".cache/subsonic/covers";

/// Default auto-scan interval (5 minutes).
const DEFAULT_AUTO_SCAN_INTERVAL_SECS: u64 = 300;

/// Scan mode controlling how files are scanned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScanMode {
    /// Full scan - re-scan all files regardless of modification time.
    Full,
    /// Incremental scan - only scan new or modified files.
    #[default]
    Incremental,
}

/// Music library scanner.
pub struct Scanner {
    pool: DbPool,
    cover_art_dir: PathBuf,
}

/// Auto-scanner that runs periodic scans in the background.
#[derive(Clone, Debug)]
pub struct AutoScanner {
    pool: DbPool,
    cover_art_dir: PathBuf,
    interval: Duration,
    scan_state: ScanStateHandle,
    shutdown_tx: Option<watch::Sender<bool>>,
}

impl Scanner {
    /// Create a new scanner.
    #[must_use]
    pub fn new(pool: DbPool) -> Self {
        // Use home directory for cover art cache
        let cover_art_dir = dirs::home_dir().map_or_else(
            || PathBuf::from(COVER_ART_CACHE_DIR),
            |h| h.join(COVER_ART_CACHE_DIR),
        );

        Self {
            pool,
            cover_art_dir,
        }
    }

    /// Create a new scanner with a custom cover art directory.
    #[must_use]
    pub const fn with_cover_art_dir(pool: DbPool, cover_art_dir: PathBuf) -> Self {
        Self {
            pool,
            cover_art_dir,
        }
    }

    /// Ensure cover art cache directory exists.
    fn ensure_cover_art_dir(&self) -> Result<(), ScanError> {
        if !self.cover_art_dir.exists() {
            fs::create_dir_all(&self.cover_art_dir)?;
        }
        Ok(())
    }

    /// Save cover art to cache and return the cover art ID.
    fn save_cover_art(&self, data: &[u8], mime: &str) -> Result<String, ScanError> {
        use md5::{Digest, Md5};

        // Generate hash-based ID for the cover art
        let mut hasher = Md5::new();
        hasher.update(data);
        let hash = hex::encode(hasher.finalize());

        // Determine file extension from MIME type
        let ext = match mime {
            "image/png" => "png",
            "image/gif" => "gif",
            "image/bmp" => "bmp",
            "image/tiff" => "tiff",
            _ => "jpg", // Default to JPEG
        };

        let filename = format!("{hash}.{ext}");
        let filepath = self.cover_art_dir.join(&filename);

        // Only write if file doesn't already exist (same content = same hash)
        if !filepath.exists() {
            fs::write(&filepath, data)?;
        }

        // Return just the hash as the cover art ID
        Ok(hash)
    }

    /// Get the cover art cache directory path.
    #[must_use]
    pub fn cover_art_dir(&self) -> &Path {
        &self.cover_art_dir
    }

    /// Look for external cover art file in the given directory.
    /// Tries common filenames like cover.jpg, folder.png, etc.
    /// Returns the cover art data and MIME type if found.
    fn find_external_cover_art(dir: &Path) -> Option<(Vec<u8>, String)> {
        // Try each common filename with each supported extension
        for filename in COVER_ART_FILENAMES {
            for ext in IMAGE_EXTENSIONS {
                let path = dir.join(format!("{filename}.{ext}"));
                if path.exists()
                    && path.is_file()
                    && let Ok(data) = fs::read(&path)
                {
                    let mime = match *ext {
                        "png" => "image/png",
                        "gif" => "image/gif",
                        "bmp" => "image/bmp",
                        "webp" => "image/webp",
                        _ => "image/jpeg",
                    };
                    return Some((data, mime.to_string()));
                }
            }
        }

        // Also try case-insensitive matching as a fallback
        // Some albums might have "Cover.JPG" or "FOLDER.PNG"
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.filter_map(std::result::Result::ok) {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }

                let filename = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(str::to_lowercase);
                let ext = path
                    .extension()
                    .and_then(|s| s.to_str())
                    .map(str::to_lowercase);

                if let (Some(name), Some(extension)) = (filename, ext)
                    && COVER_ART_FILENAMES.contains(&name.as_str())
                    && IMAGE_EXTENSIONS.contains(&extension.as_str())
                    && let Ok(data) = fs::read(&path)
                {
                    let mime = match extension.as_str() {
                        "png" => "image/png",
                        "gif" => "image/gif",
                        "bmp" => "image/bmp",
                        "webp" => "image/webp",
                        _ => "image/jpeg",
                    };
                    return Some((data, mime.to_string()));
                }
            }
        }

        None
    }

    /// Scan all enabled music folders (full scan).
    pub fn scan_all(&self) -> Result<ScanResult, ScanError> {
        self.scan_all_with_options(None, ScanMode::Full)
    }

    /// Scan all enabled music folders (incremental - only changed files).
    pub fn scan_all_incremental(&self) -> Result<ScanResult, ScanError> {
        self.scan_all_with_options(None, ScanMode::Incremental)
    }

    /// Scan all enabled music folders with optional progress tracking.
    ///
    /// If a `ScanState` is provided, the count will be updated as tracks are processed.
    pub fn scan_all_with_state(&self, state: Option<&ScanState>) -> Result<ScanResult, ScanError> {
        self.scan_all_with_options(state, ScanMode::Full)
    }

    /// Scan all enabled music folders with optional progress tracking and scan mode.
    pub fn scan_all_with_options(
        &self,
        state: Option<&ScanState>,
        mode: ScanMode,
    ) -> Result<ScanResult, ScanError> {
        let folder_repo = MusicFolderRepository::new(self.pool.clone());
        let folders = folder_repo.find_enabled()?;

        if folders.is_empty() {
            return Err(ScanError::NoMusicFolders);
        }

        let mut total_result = ScanResult::default();

        for folder in &folders {
            // Update scan state with current folder
            if let Some(s) = state {
                s.set_current_folder(Some(folder.name.clone()));
            }

            println!(
                "Scanning folder: {} ({}) [mode: {:?}]",
                folder.name, folder.path, mode
            );
            match self.scan_folder_with_options(folder, state, mode) {
                Ok(result) => {
                    total_result.tracks_found += result.tracks_found;
                    total_result.tracks_added += result.tracks_added;
                    total_result.tracks_updated += result.tracks_updated;
                    total_result.tracks_skipped += result.tracks_skipped;
                    total_result.tracks_removed += result.tracks_removed;
                    total_result.tracks_failed += result.tracks_failed;
                    total_result.artists_added += result.artists_added;
                    total_result.albums_added += result.albums_added;
                    total_result.cover_art_saved += result.cover_art_saved;
                }
                Err(e) => {
                    eprintln!("Error scanning folder {}: {}", folder.name, e);
                }
            }
        }

        // Clean up orphaned artists and albums after scanning all folders
        if let Some(s) = state {
            s.set_phase(ScanPhase::Cleaning);
            s.set_current_folder(None);
        }

        if let Err(e) = self.cleanup_orphans() {
            eprintln!("Warning: Failed to cleanup orphaned records: {e}");
        }

        Ok(total_result)
    }

    /// Scan a specific music folder by ID (full scan).
    pub fn scan_folder_by_id(&self, folder_id: i32) -> Result<ScanResult, ScanError> {
        self.scan_folder_by_id_with_mode(folder_id, ScanMode::Full)
    }

    /// Scan a specific music folder by ID with scan mode.
    pub fn scan_folder_by_id_with_mode(
        &self,
        folder_id: i32,
        mode: ScanMode,
    ) -> Result<ScanResult, ScanError> {
        let folder_repo = MusicFolderRepository::new(self.pool.clone());
        let folder = folder_repo
            .find_by_id(folder_id)?
            .ok_or_else(|| ScanError::FolderNotFound(folder_id.to_string()))?;

        println!(
            "Scanning folder: {} ({}) [mode: {:?}]",
            folder.name, folder.path, mode
        );
        self.scan_folder_with_options(&folder, None, mode)
    }

    /// Scan a single music folder with optional progress tracking and scan mode.
    fn scan_folder_with_options(
        &self,
        folder: &MusicFolder,
        state: Option<&ScanState>,
        mode: ScanMode,
    ) -> Result<ScanResult, ScanError> {
        let mut result = ScanResult::default();
        let folder_path = Path::new(&folder.path);

        if !folder_path.exists() {
            return Err(ScanError::FolderNotFound(folder.path.clone()));
        }

        // Set discovery phase
        if let Some(s) = state {
            s.set_phase(ScanPhase::Discovering);
        }

        // Get existing songs in this folder for incremental scanning
        let existing_songs = self.get_existing_songs(folder.id)?;

        // Collect all audio files on disk
        let (tracks, discovered_paths) = Self::discover_tracks_with_paths(folder_path, folder);
        result.tracks_found = tracks.len();

        // Set total count now that we know how many files to process
        if let Some(s) = state {
            // Add to total (accumulates across folders)
            let current_total = s.get_total();
            s.set_total(current_total + tracks.len() as u64);
            s.set_phase(ScanPhase::Processing);
        }

        println!("  Found {} audio files on disk", tracks.len());

        // Find deleted files (in database but not on disk)
        let deleted_paths: Vec<_> = existing_songs
            .keys()
            .filter(|path| !discovered_paths.contains(*path))
            .cloned()
            .collect();

        if !deleted_paths.is_empty() {
            println!(
                "  Removing {} deleted files from database",
                deleted_paths.len()
            );
            result.tracks_removed = self.remove_deleted_songs(&deleted_paths)?;
        }

        // Process tracks and populate database
        let (
            artists_added,
            albums_added,
            tracks_added,
            tracks_updated,
            tracks_skipped,
            tracks_failed,
            cover_art_saved,
        ) = self.process_tracks_with_options(folder, tracks, &existing_songs, state, mode)?;

        result.artists_added = artists_added;
        result.albums_added = albums_added;
        result.tracks_added = tracks_added;
        result.tracks_updated = tracks_updated;
        result.tracks_skipped = tracks_skipped;
        result.tracks_failed = tracks_failed;
        result.cover_art_saved = cover_art_saved;

        Ok(result)
    }

    /// Get existing songs in a folder from the database.
    /// Returns a map of path -> (id, `file_modified_at`).
    fn get_existing_songs(
        &self,
        folder_id: i32,
    ) -> Result<HashMap<String, (i32, Option<i64>)>, ScanError> {
        use crate::db::schema::songs;
        use diesel::prelude::*;

        let mut conn = self.pool.get().map_err(MusicRepoError::Pool)?;

        let existing: Vec<(i32, String, Option<i64>)> = songs::table
            .filter(songs::music_folder_id.eq(folder_id))
            .select((songs::id, songs::path, songs::file_modified_at))
            .load(&mut conn)
            .map_err(MusicRepoError::Database)?;

        Ok(existing
            .into_iter()
            .map(|(id, path, mtime)| (path, (id, mtime)))
            .collect())
    }

    /// Remove songs that no longer exist on disk.
    fn remove_deleted_songs(&self, paths: &[String]) -> Result<usize, ScanError> {
        use crate::db::schema::songs;
        use diesel::prelude::*;

        let mut conn = self.pool.get().map_err(MusicRepoError::Pool)?;

        let deleted = diesel::delete(songs::table.filter(songs::path.eq_any(paths)))
            .execute(&mut conn)
            .map_err(MusicRepoError::Database)?;

        Ok(deleted)
    }

    /// Clean up orphaned artists and albums (those with no songs).
    fn cleanup_orphans(&self) -> Result<(), ScanError> {
        use diesel::prelude::*;

        let mut conn = self.pool.get().map_err(MusicRepoError::Pool)?;

        // Delete albums with no songs
        diesel::sql_query(
            "DELETE FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM songs WHERE album_id IS NOT NULL)"
        )
        .execute(&mut conn)
        .map_err(MusicRepoError::Database)?;

        // Delete artists with no songs and no albums
        diesel::sql_query(
            "DELETE FROM artists WHERE id NOT IN (SELECT DISTINCT artist_id FROM songs WHERE artist_id IS NOT NULL) AND id NOT IN (SELECT DISTINCT artist_id FROM albums WHERE artist_id IS NOT NULL)"
        )
        .execute(&mut conn)
        .map_err(MusicRepoError::Database)?;

        Ok(())
    }

    /// Discover all audio files in a directory, also returning the set of discovered paths.
    /// Uses parallel processing for metadata reading.
    fn discover_tracks_with_paths(
        folder_path: &Path,
        folder: &MusicFolder,
    ) -> (Vec<ScannedTrack>, HashSet<String>) {
        // First, collect all audio file paths (fast, sequential walk)
        let audio_files: Vec<PathBuf> = WalkDir::new(folder_path)
            .follow_links(true)
            .into_iter()
            .filter_map(std::result::Result::ok)
            .filter(|entry| entry.path().is_file())
            .filter_map(|entry| {
                let path = entry.into_path();
                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_lowercase);

                match ext {
                    Some(ext) if AUDIO_EXTENSIONS.contains(&ext.as_str()) => Some(path),
                    _ => None,
                }
            })
            .collect();

        // Build paths set
        let paths: HashSet<String> = audio_files
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();

        // Read metadata in parallel using rayon
        let folder_path_str = folder.path.clone();
        let tracks: Vec<ScannedTrack> = audio_files
            .par_iter()
            .filter_map(|path| {
                let extension = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_lowercase)
                    .unwrap_or_default();

                match Self::read_track_metadata_static(path, &extension, &folder_path_str) {
                    Ok(track) => Some(track),
                    Err(e) => {
                        eprintln!("  Warning: Failed to read {}: {}", path.display(), e);
                        None
                    }
                }
            })
            .collect();

        (tracks, paths)
    }

    /// Static version of `read_track_metadata` for use with rayon (no &self needed).
    #[allow(clippy::too_many_lines)]
    fn read_track_metadata_static(
        path: &Path,
        extension: &str,
        folder_path: &str,
    ) -> Result<ScannedTrack, Box<dyn std::error::Error + Send + Sync>> {
        let metadata = fs::metadata(path)?;
        let file_size = metadata.len();

        // Get file modification time
        let file_modified_at = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| i64::try_from(d.as_secs()).unwrap_or(0));

        // Get parent path relative to music folder
        let parent_path = path
            .parent()
            .map(|p| {
                p.strip_prefix(folder_path)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .to_string()
            })
            .unwrap_or_default();

        // Read audio tags
        let tagged_file = lofty::read_from_path(path)?;

        let properties = tagged_file.properties();
        let duration_secs = u32::try_from(properties.duration().as_secs()).unwrap_or(0);
        let bit_rate = properties.audio_bitrate();
        let bit_depth = properties.bit_depth();
        let sample_rate = properties.sample_rate();
        let channels = properties.channels();

        // Get tags (try primary tag first, then any available)
        let tag = tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag());

        let (
            title,
            artist,
            album,
            album_artist,
            track_number,
            disc_number,
            year,
            genre,
            cover_art_data,
            cover_art_mime,
        ) = tag.map_or(
            (None, None, None, None, None, None, None, None, None, None),
            |tag| {
                // Extract embedded cover art (first picture)
                let (art_data, art_mime) = tag.pictures().first().map_or((None, None), |p| {
                    let mime = match p.mime_type() {
                        Some(lofty::picture::MimeType::Png) => "image/png",
                        Some(lofty::picture::MimeType::Gif) => "image/gif",
                        Some(lofty::picture::MimeType::Bmp) => "image/bmp",
                        Some(lofty::picture::MimeType::Tiff) => "image/tiff",
                        _ => "image/jpeg", // Default to JPEG
                    };
                    (Some(p.data().to_vec()), Some(mime.to_string()))
                });

                (
                    tag.title().map(|s| s.to_string()),
                    tag.artist().map(|s| s.to_string()),
                    tag.album().map(|s| s.to_string()),
                    tag.get_string(&ItemKey::AlbumArtist)
                        .map(std::string::ToString::to_string),
                    tag.track(),
                    tag.disk(),
                    tag.year(),
                    tag.genre().map(|s| s.to_string()),
                    art_data,
                    art_mime,
                )
            },
        );

        // Use filename as title if no tag
        let title = title.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string()
        });

        let content_type = match extension {
            "mp3" => "audio/mpeg",
            "flac" => "audio/flac",
            "ogg" => "audio/ogg",
            "opus" => "audio/opus",
            "m4a" | "aac" => "audio/mp4",
            "wav" => "audio/wav",
            "wma" => "audio/x-ms-wma",
            "aiff" => "audio/aiff",
            "ape" => "audio/ape",
            "wv" => "audio/wavpack",
            _ => "audio/unknown",
        }
        .to_string();

        Ok(ScannedTrack {
            path: path.to_path_buf(),
            parent_path,
            file_size,
            content_type,
            suffix: extension.to_string(),
            title,
            artist,
            album,
            album_artist,
            track_number,
            disc_number,
            year,
            genre,
            duration_secs,
            bit_rate,
            bit_depth,
            sample_rate,
            channels,
            cover_art_data,
            cover_art_mime,
            file_modified_at,
        })
    }

    /// Process scanned tracks and populate the database with options.
    /// Returns (`artists_added`, `albums_added`, `tracks_added`, `tracks_updated`, `tracks_skipped`, `tracks_failed`, `cover_art_saved`)
    #[allow(clippy::type_complexity, clippy::too_many_lines)]
    fn process_tracks_with_options(
        &self,
        folder: &MusicFolder,
        tracks: Vec<ScannedTrack>,
        existing_songs: &HashMap<String, (i32, Option<i64>)>,
        state: Option<&ScanState>,
        mode: ScanMode,
    ) -> Result<(usize, usize, usize, usize, usize, usize, usize), ScanError> {
        use crate::db::schema::{albums, artists, songs};
        use diesel::prelude::*;

        // Ensure cover art directory exists
        self.ensure_cover_art_dir()?;

        let mut conn = self.pool.get().map_err(MusicRepoError::Pool)?;

        // Pre-load all existing artists into cache (much faster than individual lookups)
        let mut artist_cache: HashMap<String, i32> = artists::table
            .select((artists::name, artists::id))
            .load::<(String, i32)>(&mut conn)
            .map_err(MusicRepoError::Database)?
            .into_iter()
            .collect();

        // Pre-load all existing albums into cache
        let mut album_cache: HashMap<(String, Option<i32>), i32> = albums::table
            .select((albums::name, albums::artist_id, albums::id))
            .load::<(String, Option<i32>, i32)>(&mut conn)
            .map_err(MusicRepoError::Database)?
            .into_iter()
            .map(|(name, artist_id, id)| ((name, artist_id), id))
            .collect();

        // Pre-load album cover art hashes
        let mut album_cover_art_cache: HashMap<i32, Option<String>> = albums::table
            .select((albums::id, albums::cover_art))
            .load::<(i32, Option<String>)>(&mut conn)
            .map_err(MusicRepoError::Database)?
            .into_iter()
            .collect();

        // Cache for external cover art per directory (None = already checked, no cover art found)
        let mut dir_cover_art_cache: HashMap<PathBuf, Option<(Vec<u8>, String)>> = HashMap::new();

        let mut artists_added = 0;
        let mut albums_added = 0;
        let mut tracks_added = 0;
        let mut tracks_updated = 0;
        let mut tracks_skipped = 0;
        let tracks_failed = 0;
        let mut cover_art_saved = 0;

        // Collect unique new artists and albums first (avoid duplicate inserts)
        let mut new_artists: HashSet<String> = HashSet::new();

        // First pass: collect all unique new artists
        for track in &tracks {
            let path_str = track.path.to_string_lossy().to_string();

            // Skip unchanged files in incremental mode
            if mode == ScanMode::Incremental
                && let Some((_, stored_mtime)) = existing_songs.get(&path_str)
                && let (Some(stored), Some(current)) = (stored_mtime, track.file_modified_at)
                && *stored == current
            {
                continue;
            }

            let artist_name = track
                .album_artist
                .as_ref()
                .or(track.artist.as_ref())
                .cloned();

            if let Some(ref name) = artist_name
                && !artist_cache.contains_key(name)
            {
                new_artists.insert(name.clone());
            }
        }

        // Batch insert new artists in a transaction
        if !new_artists.is_empty() {
            conn.transaction::<_, diesel::result::Error, _>(|conn| {
                for name in &new_artists {
                    diesel::insert_into(artists::table)
                        .values(artists::name.eq(name))
                        .on_conflict_do_nothing()
                        .execute(conn)?;
                }
                Ok(())
            })
            .map_err(MusicRepoError::Database)?;

            // Reload artist cache to get new IDs
            let new_artist_ids: Vec<(String, i32)> = artists::table
                .filter(artists::name.eq_any(&new_artists))
                .select((artists::name, artists::id))
                .load(&mut conn)
                .map_err(MusicRepoError::Database)?;

            for (name, id) in new_artist_ids {
                if !artist_cache.contains_key(&name) {
                    artists_added += 1;
                }
                artist_cache.insert(name, id);
            }
        }

        let mut prepared_tracks: Vec<PreparedTrack> = Vec::with_capacity(tracks.len());

        // Second pass: resolve albums and prepare tracks
        for track in tracks {
            let path_str = track.path.to_string_lossy().to_string();

            // For incremental scan, check if file has changed
            if mode == ScanMode::Incremental
                && let Some((_, stored_mtime)) = existing_songs.get(&path_str)
                && let (Some(stored), Some(current)) = (stored_mtime, track.file_modified_at)
                && *stored == current
            {
                // File hasn't changed, skip processing
                tracks_skipped += 1;
                if let Some(state) = state {
                    state.increment_count();
                }
                continue;
            }

            // Get artist ID from cache
            let artist_name = track
                .album_artist
                .as_ref()
                .or(track.artist.as_ref())
                .cloned();

            let artist_id = artist_name
                .as_ref()
                .and_then(|name| artist_cache.get(name).copied());

            // Get or create album
            let album_id = if let Some(ref album_name) = track.album {
                let cache_key = (album_name.clone(), artist_id);

                if let Some(&id) = album_cache.get(&cache_key) {
                    Some(id)
                } else {
                    // Insert new album
                    diesel::insert_into(albums::table)
                        .values((
                            albums::name.eq(album_name),
                            albums::artist_id.eq(artist_id),
                            albums::artist_name.eq(&artist_name),
                            albums::year.eq(track.year.map(|y| i32::try_from(y).unwrap_or(0))),
                            albums::genre.eq(&track.genre),
                        ))
                        .on_conflict_do_nothing()
                        .execute(&mut conn)
                        .map_err(MusicRepoError::Database)?;

                    // Get the album ID
                    let mut query = albums::table
                        .filter(albums::name.eq(album_name))
                        .into_boxed();
                    if let Some(aid) = artist_id {
                        query = query.filter(albums::artist_id.eq(aid));
                    } else {
                        query = query.filter(albums::artist_id.is_null());
                    }

                    let album_row: Option<(i32, Option<String>)> = query
                        .select((albums::id, albums::cover_art))
                        .first(&mut conn)
                        .optional()
                        .map_err(MusicRepoError::Database)?;

                    if let Some((id, existing_cover)) = album_row {
                        if !album_cache.contains_key(&cache_key) {
                            albums_added += 1;
                        }
                        album_cache.insert(cache_key, id);
                        album_cover_art_cache.insert(id, existing_cover);
                        Some(id)
                    } else {
                        None
                    }
                }
            } else {
                None
            };

            // Handle cover art
            let album_cover_art_id = if let Some(album_id) = album_id {
                let existing_cover_art = album_cover_art_cache.get(&album_id).cloned().flatten();

                if existing_cover_art.is_none() {
                    let art_source: Option<(Vec<u8>, String)> =
                        if let (Some(art_data), Some(art_mime)) =
                            (&track.cover_art_data, &track.cover_art_mime)
                        {
                            Some((art_data.clone(), art_mime.clone()))
                        } else if let Some(parent_dir) = track.path.parent() {
                            let parent_buf = parent_dir.to_path_buf();
                            dir_cover_art_cache
                                .entry(parent_buf)
                                .or_insert_with(|| Self::find_external_cover_art(parent_dir))
                                .clone()
                        } else {
                            None
                        };

                    if let Some((art_data, art_mime)) = art_source {
                        match self.save_cover_art(&art_data, &art_mime) {
                            Ok(cover_art_hash) => {
                                if let Err(e) =
                                    diesel::update(albums::table.filter(albums::id.eq(album_id)))
                                        .set(albums::cover_art.eq(&cover_art_hash))
                                        .execute(&mut conn)
                                {
                                    eprintln!("  Warning: Failed to update album cover art: {e}");
                                    None
                                } else {
                                    album_cover_art_cache
                                        .insert(album_id, Some(cover_art_hash.clone()));
                                    cover_art_saved += 1;
                                    Some(cover_art_hash)
                                }
                            }
                            Err(e) => {
                                eprintln!("  Warning: Failed to save cover art: {e}");
                                None
                            }
                        }
                    } else {
                        None
                    }
                } else {
                    existing_cover_art
                }
            } else {
                None
            };

            let is_update = existing_songs.contains_key(&path_str);

            prepared_tracks.push(PreparedTrack {
                track,
                path_str,
                artist_id,
                album_id,
                cover_art: album_cover_art_id,
                is_update,
            });
        }

        // Process songs in batches within transactions
        for batch in prepared_tracks.chunks(BATCH_SIZE) {
            conn.transaction::<_, diesel::result::Error, _>(|conn| {
                for prepared in batch {
                    let result = if prepared.is_update {
                        diesel::update(songs::table.filter(songs::path.eq(&prepared.path_str)))
                            .set((
                                songs::title.eq(&prepared.track.title),
                                songs::album_id.eq(prepared.album_id),
                                songs::artist_id.eq(prepared.artist_id),
                                songs::artist_name.eq(&prepared.track.artist),
                                songs::album_name.eq(&prepared.track.album),
                                songs::file_size
                                    .eq(i64::try_from(prepared.track.file_size).unwrap_or(0)),
                                songs::duration
                                    .eq(i32::try_from(prepared.track.duration_secs).unwrap_or(0)),
                                songs::bit_rate.eq(prepared
                                    .track
                                    .bit_rate
                                    .map(|b| i32::try_from(b).unwrap_or(0))),
                                songs::bit_depth.eq(prepared.track.bit_depth.map(i32::from)),
                                songs::sampling_rate.eq(prepared
                                    .track
                                    .sample_rate
                                    .map(|s| i32::try_from(s).unwrap_or(0))),
                                songs::channel_count.eq(prepared.track.channels.map(i32::from)),
                                songs::track_number.eq(prepared
                                    .track
                                    .track_number
                                    .map(|t| i32::try_from(t).unwrap_or(0))),
                                songs::disc_number.eq(prepared
                                    .track
                                    .disc_number
                                    .map(|d| i32::try_from(d).unwrap_or(0))),
                                songs::year
                                    .eq(prepared.track.year.map(|y| i32::try_from(y).unwrap_or(0))),
                                songs::genre.eq(&prepared.track.genre),
                                songs::cover_art.eq(&prepared.cover_art),
                                songs::file_modified_at.eq(prepared.track.file_modified_at),
                                songs::updated_at.eq(diesel::dsl::now),
                            ))
                            .execute(conn)
                    } else {
                        diesel::insert_into(songs::table)
                            .values((
                                songs::title.eq(&prepared.track.title),
                                songs::album_id.eq(prepared.album_id),
                                songs::artist_id.eq(prepared.artist_id),
                                songs::artist_name.eq(&prepared.track.artist),
                                songs::album_name.eq(&prepared.track.album),
                                songs::music_folder_id.eq(folder.id),
                                songs::path.eq(&prepared.path_str),
                                songs::parent_path.eq(&prepared.track.parent_path),
                                songs::file_size
                                    .eq(i64::try_from(prepared.track.file_size).unwrap_or(0)),
                                songs::content_type.eq(&prepared.track.content_type),
                                songs::suffix.eq(&prepared.track.suffix),
                                songs::duration
                                    .eq(i32::try_from(prepared.track.duration_secs).unwrap_or(0)),
                                songs::bit_rate.eq(prepared
                                    .track
                                    .bit_rate
                                    .map(|b| i32::try_from(b).unwrap_or(0))),
                                songs::bit_depth.eq(prepared.track.bit_depth.map(i32::from)),
                                songs::sampling_rate.eq(prepared
                                    .track
                                    .sample_rate
                                    .map(|s| i32::try_from(s).unwrap_or(0))),
                                songs::channel_count.eq(prepared.track.channels.map(i32::from)),
                                songs::track_number.eq(prepared
                                    .track
                                    .track_number
                                    .map(|t| i32::try_from(t).unwrap_or(0))),
                                songs::disc_number.eq(prepared
                                    .track
                                    .disc_number
                                    .map(|d| i32::try_from(d).unwrap_or(0))),
                                songs::year
                                    .eq(prepared.track.year.map(|y| i32::try_from(y).unwrap_or(0))),
                                songs::genre.eq(&prepared.track.genre),
                                songs::cover_art.eq(&prepared.cover_art),
                                songs::file_modified_at.eq(prepared.track.file_modified_at),
                            ))
                            .execute(conn)
                    };

                    match result {
                        Ok(_) => {}
                        Err(e) => {
                            eprintln!("  Failed to insert {}: {}", prepared.path_str, e);
                        }
                    }
                }
                Ok(())
            })
            .map_err(MusicRepoError::Database)?;

            // Update counters and progress
            for prepared in batch {
                if prepared.is_update {
                    tracks_updated += 1;
                } else {
                    tracks_added += 1;
                }
                if let Some(state) = state {
                    state.increment_count();
                }
            }
        }

        // Update album song counts and durations
        Self::update_album_stats(&mut conn)?;

        Ok((
            artists_added,
            albums_added,
            tracks_added,
            tracks_updated,
            tracks_skipped,
            tracks_failed,
            cover_art_saved,
        ))
    }

    /// Update album statistics (song count, duration) based on songs.
    fn update_album_stats(conn: &mut diesel::SqliteConnection) -> Result<(), ScanError> {
        use diesel::prelude::*;

        // This updates each album's song_count and duration based on its songs
        diesel::sql_query(
            r"
            UPDATE albums SET
                song_count = (SELECT COUNT(*) FROM songs WHERE songs.album_id = albums.id),
                duration = (SELECT COALESCE(SUM(duration), 0) FROM songs WHERE songs.album_id = albums.id),
                updated_at = CURRENT_TIMESTAMP
            ",
        )
        .execute(conn)
        .map_err(MusicRepoError::Database)?;

        Ok(())
    }
}

impl AutoScanner {
    /// Create a new auto-scanner with default interval (5 minutes).
    #[must_use]
    pub fn new(pool: DbPool, scan_state: ScanStateHandle) -> Self {
        let cover_art_dir = dirs::home_dir().map_or_else(
            || PathBuf::from(COVER_ART_CACHE_DIR),
            |h| h.join(COVER_ART_CACHE_DIR),
        );

        Self {
            pool,
            cover_art_dir,
            interval: Duration::from_secs(DEFAULT_AUTO_SCAN_INTERVAL_SECS),
            scan_state,
            shutdown_tx: None,
        }
    }

    /// Create a new auto-scanner with a custom interval.
    #[must_use]
    pub fn with_interval(pool: DbPool, scan_state: ScanStateHandle, interval_secs: u64) -> Self {
        let cover_art_dir = dirs::home_dir().map_or_else(
            || PathBuf::from(COVER_ART_CACHE_DIR),
            |h| h.join(COVER_ART_CACHE_DIR),
        );

        Self {
            pool,
            cover_art_dir,
            interval: Duration::from_secs(interval_secs),
            scan_state,
            shutdown_tx: None,
        }
    }

    /// Start the auto-scanner in the background.
    /// Returns a handle that can be used to stop the scanner.
    pub fn start(&mut self) -> AutoScanHandle {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        self.shutdown_tx = Some(shutdown_tx.clone());

        let pool = self.pool.clone();
        let cover_art_dir = self.cover_art_dir.clone();
        let interval = self.interval;
        let scan_state = self.scan_state.clone();

        tokio::spawn(async move {
            Self::run_scan_loop(pool, cover_art_dir, interval, scan_state, shutdown_rx).await;
        });

        AutoScanHandle { shutdown_tx }
    }

    /// Run the scan loop.
    async fn run_scan_loop(
        pool: DbPool,
        cover_art_dir: PathBuf,
        interval: Duration,
        scan_state: ScanStateHandle,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        tracing::info!(interval = ?interval, "Auto-scanner started");

        loop {
            // Wait for the interval or shutdown signal
            tokio::select! {
                () = tokio::time::sleep(interval) => {
                    // Time to scan
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("Auto-scanner received shutdown signal");
                        break;
                    }
                }
            }

            // Try to start a scan (skip if one is already in progress)
            if !scan_state.try_start() {
                tracing::debug!("Skipping auto-scan: scan already in progress");
                continue;
            }

            tracing::info!(scan_mode = "incremental", "Starting auto-scan");
            scan_state.reset_count();

            // Run the scan in a blocking task since it uses diesel
            let pool_clone = pool.clone();
            let cover_art_dir_clone = cover_art_dir.clone();
            let scan_state_clone = scan_state.clone();
            let result = tokio::task::spawn_blocking(move || {
                let scanner = Scanner::with_cover_art_dir(pool_clone, cover_art_dir_clone);
                scanner.scan_all_with_options(Some(scan_state_clone.get()), ScanMode::Incremental)
            })
            .await;

            scan_state.finish();

            match result {
                Ok(Ok(stats)) => {
                    tracing::info!(
                        tracks.found = stats.tracks_found,
                        tracks.added = stats.tracks_added,
                        tracks.updated = stats.tracks_updated,
                        tracks.skipped = stats.tracks_skipped,
                        tracks.removed = stats.tracks_removed,
                        tracks.failed = stats.tracks_failed,
                        "Auto-scan complete"
                    );
                }
                Ok(Err(ScanError::NoMusicFolders)) => {
                    tracing::debug!("Auto-scan skipped: no music folders configured");
                }
                Ok(Err(e)) => {
                    tracing::error!(error = %e, "Auto-scan failed");
                }
                Err(e) => {
                    tracing::error!(error = %e, "Auto-scan task panicked");
                }
            }
        }

        tracing::info!("Auto-scanner stopped");
    }
}

/// Handle for controlling the auto-scanner.
pub struct AutoScanHandle {
    shutdown_tx: watch::Sender<bool>,
}

impl AutoScanHandle {
    /// Stop the auto-scanner.
    pub fn stop(&self) {
        let _ = self.shutdown_tx.send(true);
    }
}
