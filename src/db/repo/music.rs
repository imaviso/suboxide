//! Music library repository operations.

use chrono::NaiveDateTime;
use diesel::prelude::*;

use crate::db::DbPool;
use crate::db::repo::error::{MusicRepoError, MusicRepoErrorKind};
use crate::db::schema::{albums, artists, music_folders, songs};
use crate::models::music::{Album, Artist, MusicFolder, NewMusicFolder, Song};

// ============================================================================
// MusicFolder Repository
// ============================================================================

/// Database row representation for music folders.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = music_folders)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct MusicFolderRow {
    pub id: i32,
    pub name: String,
    pub path: String,
    pub enabled: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<MusicFolderRow> for MusicFolder {
    fn from(row: MusicFolderRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            path: row.path,
            enabled: row.enabled,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Data for inserting a new music folder.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = music_folders)]
pub struct NewMusicFolderRow<'a> {
    pub name: &'a str,
    pub path: &'a str,
    pub enabled: bool,
}

impl<'a> From<&'a NewMusicFolder> for NewMusicFolderRow<'a> {
    fn from(folder: &'a NewMusicFolder) -> Self {
        Self {
            name: &folder.name,
            path: &folder.path,
            enabled: folder.enabled,
        }
    }
}

/// Repository for music folder database operations.
#[derive(Clone, Debug)]
pub struct MusicFolderRepository {
    pool: DbPool,
}

impl MusicFolderRepository {
    /// Create a new music folder repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get all music folders.
    pub fn find_all(&self) -> Result<Vec<MusicFolder>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = music_folders::table
            .select(MusicFolderRow::as_select())
            .order(music_folders::name.asc())
            .load(&mut conn)?;

        Ok(results.into_iter().map(MusicFolder::from).collect())
    }

    /// Get all enabled music folders.
    pub fn find_enabled(&self) -> Result<Vec<MusicFolder>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = music_folders::table
            .filter(music_folders::enabled.eq(true))
            .select(MusicFolderRow::as_select())
            .order(music_folders::name.asc())
            .load(&mut conn)?;

        Ok(results.into_iter().map(MusicFolder::from).collect())
    }

    /// Find a music folder by ID.
    pub fn find_by_id(&self, folder_id: i32) -> Result<Option<MusicFolder>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = music_folders::table
            .filter(music_folders::id.eq(folder_id))
            .select(MusicFolderRow::as_select())
            .first(&mut conn)
            .optional()?;

        Ok(result.map(MusicFolder::from))
    }

    /// Find a music folder by path.
    pub fn find_by_path(&self, path: &str) -> Result<Option<MusicFolder>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = music_folders::table
            .filter(music_folders::path.eq(path))
            .select(MusicFolderRow::as_select())
            .first(&mut conn)
            .optional()?;

        Ok(result.map(MusicFolder::from))
    }

    /// Create a new music folder.
    pub fn create(&self, new_folder: &NewMusicFolder) -> Result<MusicFolder, MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Check if path already exists
        let existing = music_folders::table
            .filter(music_folders::path.eq(&new_folder.path))
            .count()
            .get_result::<i64>(&mut conn)?;

        if existing > 0 {
            return Err(MusicRepoError::new(
                MusicRepoErrorKind::AlreadyExists,
                new_folder.path.clone(),
            ));
        }

        let row: NewMusicFolderRow = new_folder.into();
        diesel::insert_into(music_folders::table)
            .values(&row)
            .execute(&mut conn)?;

        // Fetch the created folder
        let folder = music_folders::table
            .filter(music_folders::path.eq(&new_folder.path))
            .select(MusicFolderRow::as_select())
            .first(&mut conn)?;

        Ok(MusicFolder::from(folder))
    }

    /// Delete a music folder by ID.
    pub fn delete(&self, folder_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let deleted = diesel::delete(music_folders::table.filter(music_folders::id.eq(folder_id)))
            .execute(&mut conn)?;

        Ok(deleted > 0)
    }

    /// Enable or disable a music folder.
    pub fn set_enabled(&self, folder_id: i32, enabled: bool) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let updated = diesel::update(music_folders::table.filter(music_folders::id.eq(folder_id)))
            .set(music_folders::enabled.eq(enabled))
            .execute(&mut conn)?;

        Ok(updated > 0)
    }
}

// ============================================================================
// Artist Repository
// ============================================================================

/// Database row representation for artists.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = artists)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ArtistRow {
    pub id: i32,
    pub name: String,
    pub sort_name: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub cover_art: Option<String>,
    pub artist_image_url: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<ArtistRow> for Artist {
    fn from(row: ArtistRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            sort_name: row.sort_name,
            musicbrainz_id: row.musicbrainz_id,
            cover_art: row.cover_art,
            artist_image_url: row.artist_image_url,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Repository for artist database operations.
#[derive(Clone, Debug)]
pub struct ArtistRepository {
    pool: DbPool,
}

impl ArtistRepository {
    /// Create a new artist repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get all artists.
    pub fn find_all(&self) -> Result<Vec<Artist>, MusicRepoError> {
        let mut conn = self.pool.get().map_err(MusicRepoError::from)?;

        let results = artists::table
            .select(ArtistRow::as_select())
            .order(artists::name.asc())
            .load(&mut conn)
            .map_err(MusicRepoError::from)?;

        Ok(results.into_iter().map(Artist::from).collect())
    }

    /// Find an artist by ID.
    pub fn find_by_id(&self, artist_id: i32) -> Result<Option<Artist>, MusicRepoError> {
        let mut conn = self.pool.get().map_err(MusicRepoError::from)?;

        let result = artists::table
            .filter(artists::id.eq(artist_id))
            .select(ArtistRow::as_select())
            .first(&mut conn)
            .optional()
            .map_err(MusicRepoError::from)?;

        Ok(result.map(Artist::from))
    }

    /// Find an artist by name.
    pub fn find_by_name(&self, name: &str) -> Result<Option<Artist>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = artists::table
            .filter(artists::name.eq(name))
            .select(ArtistRow::as_select())
            .first(&mut conn)
            .optional()?;

        Ok(result.map(Artist::from))
    }

    /// Count albums for an artist.
    pub fn count_albums(&self, artist_id: i32) -> Result<i64, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let count = albums::table
            .filter(albums::artist_id.eq(artist_id))
            .count()
            .get_result(&mut conn)?;

        Ok(count)
    }

    /// Get the most recent update time for any artist.
    pub fn get_last_modified(&self) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = artists::table
            .select(diesel::dsl::max(artists::updated_at))
            .first(&mut conn)?;

        Ok(result)
    }

    /// Count albums for multiple artists in a single query.
    /// Returns a `HashMap` mapping `artist_id` to `album_count`.
    pub fn count_albums_batch(
        &self,
        artist_ids: &[i32],
    ) -> Result<std::collections::HashMap<i32, i64>, MusicRepoError> {
        use std::collections::HashMap;

        if artist_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = self.pool.get()?;

        let counts: Vec<(i32, i64)> = albums::table
            .filter(albums::artist_id.eq_any(artist_ids))
            .group_by(albums::artist_id)
            .select((
                albums::artist_id.assume_not_null(),
                diesel::dsl::count_star(),
            ))
            .load(&mut conn)?;

        Ok(counts.into_iter().collect())
    }

    /// Search artists by name with pagination.
    /// An empty query returns all artists.
    pub fn search(
        &self,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Artist>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        if query.is_empty() {
            // Return all artists
            let results = artists::table
                .select(ArtistRow::as_select())
                .order(artists::name.asc())
                .offset(offset)
                .limit(limit)
                .load(&mut conn)?;
            return Ok(results.into_iter().map(Artist::from).collect());
        }

        let pattern = format!("%{query}%");
        let results = artists::table
            .filter(artists::name.like(&pattern))
            .select(ArtistRow::as_select())
            .order(artists::name.asc())
            .offset(offset)
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Artist::from).collect())
    }
}

// ============================================================================
// Album Repository
// ============================================================================

/// Database row representation for albums.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = albums)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct AlbumRow {
    pub id: i32,
    pub name: String,
    pub sort_name: Option<String>,
    pub artist_id: Option<i32>,
    pub artist_name: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_art: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub duration: i32,
    pub song_count: i32,
    pub play_count: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<AlbumRow> for Album {
    fn from(row: AlbumRow) -> Self {
        Self {
            id: row.id,
            name: row.name,
            sort_name: row.sort_name,
            artist_id: row.artist_id,
            artist_name: row.artist_name,
            year: row.year,
            genre: row.genre,
            cover_art: row.cover_art,
            musicbrainz_id: row.musicbrainz_id,
            duration: row.duration,
            song_count: row.song_count,
            play_count: row.play_count,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Repository for album database operations.
#[derive(Clone, Debug)]
pub struct AlbumRepository {
    pool: DbPool,
}

impl AlbumRepository {
    /// Create a new album repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get all albums ordered by name.
    pub fn find_all(&self) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = albums::table
            .select(AlbumRow::as_select())
            .order(albums::name.asc())
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }

    /// Find an album by ID.
    pub fn find_by_id(&self, album_id: i32) -> Result<Option<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = albums::table
            .filter(albums::id.eq(album_id))
            .select(AlbumRow::as_select())
            .first(&mut conn)
            .optional()?;

        Ok(result.map(Album::from))
    }

    /// Find albums by artist ID.
    pub fn find_by_artist(&self, artist_id: i32) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = albums::table
            .filter(albums::artist_id.eq(artist_id))
            .select(AlbumRow::as_select())
            .order(albums::year.asc())
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }

    /// Find albums ordered alphabetically by name with pagination.
    pub fn find_alphabetical_by_name(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = albums::table
            .select(AlbumRow::as_select())
            .order(albums::name.asc())
            .offset(offset)
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }

    /// Find albums ordered alphabetically by artist name with pagination.
    pub fn find_alphabetical_by_artist(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = albums::table
            .select(AlbumRow::as_select())
            .order((albums::artist_name.asc(), albums::name.asc()))
            .offset(offset)
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }

    /// Find newest albums (by `created_at`) with pagination.
    pub fn find_newest(&self, offset: i64, limit: i64) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = albums::table
            .select(AlbumRow::as_select())
            .order(albums::created_at.desc())
            .offset(offset)
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }

    /// Find most frequently played albums with pagination.
    pub fn find_frequent(&self, offset: i64, limit: i64) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = albums::table
            .select(AlbumRow::as_select())
            .order(albums::play_count.desc())
            .offset(offset)
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }

    /// Find recently played albums with pagination.
    /// Note: Using `updated_at` as a proxy for last played time.
    pub fn find_recent(&self, offset: i64, limit: i64) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = albums::table
            .filter(albums::play_count.gt(0))
            .select(AlbumRow::as_select())
            .order(albums::updated_at.desc())
            .offset(offset)
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }

    /// Find random albums.
    pub fn find_random(&self, limit: i64) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        // SQLite uses RANDOM() for random ordering
        let results = albums::table
            .select(AlbumRow::as_select())
            .order(diesel::dsl::sql::<diesel::sql_types::Integer>("RANDOM()"))
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }

    /// Find albums by year range with pagination.
    pub fn find_by_year_range(
        &self,
        from_year: i32,
        to_year: i32,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = albums::table
            .filter(albums::year.ge(from_year))
            .filter(albums::year.le(to_year))
            .select(AlbumRow::as_select())
            .order(albums::year.asc())
            .offset(offset)
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }

    /// Find albums by genre with pagination.
    pub fn find_by_genre(
        &self,
        genre: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = albums::table
            .filter(albums::genre.eq(genre))
            .select(AlbumRow::as_select())
            .order(albums::name.asc())
            .offset(offset)
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }

    /// Search albums by name with pagination.
    /// An empty query returns all albums.
    pub fn search(
        &self,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        if query.is_empty() {
            // Return all albums
            let results = albums::table
                .select(AlbumRow::as_select())
                .order(albums::name.asc())
                .offset(offset)
                .limit(limit)
                .load(&mut conn)?;
            return Ok(results.into_iter().map(Album::from).collect());
        }

        let pattern = format!("%{query}%");
        let results = albums::table
            .filter(albums::name.like(&pattern))
            .select(AlbumRow::as_select())
            .order(albums::name.asc())
            .offset(offset)
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }

    /// Find albums by IDs.
    pub fn find_by_ids(&self, album_ids: &[i32]) -> Result<Vec<Album>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = albums::table
            .filter(albums::id.eq_any(album_ids))
            .select(AlbumRow::as_select())
            .load(&mut conn)?;

        Ok(results.into_iter().map(Album::from).collect())
    }
}

// ============================================================================
// Song Repository
// ============================================================================

/// Database row representation for songs.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = songs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct SongRow {
    pub id: i32,
    pub title: String,
    pub sort_name: Option<String>,
    pub album_id: Option<i32>,
    pub artist_id: Option<i32>,
    pub artist_name: Option<String>,
    pub album_name: Option<String>,
    pub music_folder_id: i32,
    pub path: String,
    pub parent_path: String,
    pub file_size: i64,
    pub content_type: String,
    pub suffix: String,
    pub duration: i32,
    pub bit_rate: Option<i32>,
    pub bit_depth: Option<i32>,
    pub sampling_rate: Option<i32>,
    pub channel_count: Option<i32>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_art: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub play_count: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

impl From<SongRow> for Song {
    fn from(row: SongRow) -> Self {
        Self {
            id: row.id,
            title: row.title,
            sort_name: row.sort_name,
            album_id: row.album_id,
            artist_id: row.artist_id,
            artist_name: row.artist_name,
            album_name: row.album_name,
            music_folder_id: row.music_folder_id,
            path: row.path,
            parent_path: row.parent_path,
            file_size: row.file_size,
            content_type: row.content_type,
            suffix: row.suffix,
            duration: row.duration,
            bit_rate: row.bit_rate,
            bit_depth: row.bit_depth,
            sampling_rate: row.sampling_rate,
            channel_count: row.channel_count,
            track_number: row.track_number,
            disc_number: row.disc_number,
            year: row.year,
            genre: row.genre,
            cover_art: row.cover_art,
            musicbrainz_id: row.musicbrainz_id,
            play_count: row.play_count,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

/// Repository for song database operations.
#[derive(Clone, Debug)]
pub struct SongRepository {
    pool: DbPool,
}

impl SongRepository {
    /// Create a new song repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Find a song by ID.
    pub fn find_by_id(&self, song_id: i32) -> Result<Option<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = songs::table
            .filter(songs::id.eq(song_id))
            .select(SongRow::as_select())
            .first(&mut conn)
            .optional()?;

        Ok(result.map(Song::from))
    }

    /// Find songs by album ID.
    pub fn find_by_album(&self, album_id: i32) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = songs::table
            .filter(songs::album_id.eq(album_id))
            .select(SongRow::as_select())
            .order((songs::disc_number.asc(), songs::track_number.asc()))
            .load(&mut conn)?;

        Ok(results.into_iter().map(Song::from).collect())
    }

    /// Find songs by artist ID.
    pub fn find_by_artist(&self, artist_id: i32) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = songs::table
            .filter(songs::artist_id.eq(artist_id))
            .select(SongRow::as_select())
            .order(songs::title.asc())
            .load(&mut conn)?;

        Ok(results.into_iter().map(Song::from).collect())
    }

    /// Find songs by music folder ID.
    pub fn find_by_music_folder(&self, folder_id: i32) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = songs::table
            .filter(songs::music_folder_id.eq(folder_id))
            .select(SongRow::as_select())
            .order(songs::path.asc())
            .load(&mut conn)?;

        Ok(results.into_iter().map(Song::from).collect())
    }

    /// Search songs by title with pagination.
    /// An empty query returns all songs.
    pub fn search(
        &self,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        if query.is_empty() {
            // Return all songs
            let results = songs::table
                .select(SongRow::as_select())
                .order(songs::title.asc())
                .offset(offset)
                .limit(limit)
                .load(&mut conn)?;
            return Ok(results.into_iter().map(Song::from).collect());
        }

        let pattern = format!("%{query}%");
        let results = songs::table
            .filter(songs::title.like(&pattern))
            .select(SongRow::as_select())
            .order(songs::title.asc())
            .offset(offset)
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Song::from).collect())
    }

    /// Get all genres with song and album counts.
    /// Returns a vector of (`genre_name`, `song_count`, `album_count`).
    pub fn get_genres(&self) -> Result<Vec<(String, i64, i64)>, MusicRepoError> {
        use std::collections::HashMap;

        let mut conn = self.pool.get()?;

        // Get song counts per genre
        let song_counts: Vec<(Option<String>, i64)> = songs::table
            .filter(songs::genre.is_not_null())
            .group_by(songs::genre)
            .select((songs::genre, diesel::dsl::count_star()))
            .load(&mut conn)?;

        // Get album counts per genre
        let album_counts: Vec<(Option<String>, i64)> = albums::table
            .filter(albums::genre.is_not_null())
            .group_by(albums::genre)
            .select((albums::genre, diesel::dsl::count_star()))
            .load(&mut conn)?;

        // Merge into a single list
        let mut genre_map: HashMap<String, (i64, i64)> = HashMap::new();

        for (genre, count) in song_counts {
            if let Some(g) = genre {
                genre_map.entry(g).or_insert((0, 0)).0 = count;
            }
        }

        for (genre, count) in album_counts {
            if let Some(g) = genre {
                genre_map.entry(g).or_insert((0, 0)).1 = count;
            }
        }

        let mut genres: Vec<(String, i64, i64)> = genre_map
            .into_iter()
            .map(|(name, (song_count, album_count))| (name, song_count, album_count))
            .collect();

        genres.sort_by(|a, b| a.0.cmp(&b.0));
        Ok(genres)
    }

    /// Find random songs with optional filters.
    pub fn find_random(
        &self,
        size: i64,
        genre: Option<&str>,
        from_year: Option<i32>,
        to_year: Option<i32>,
        music_folder_id: Option<i32>,
    ) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let mut query = songs::table.into_boxed();

        if let Some(g) = genre {
            query = query.filter(songs::genre.eq(g));
        }

        if let Some(from) = from_year {
            query = query.filter(songs::year.ge(from));
        }

        if let Some(to) = to_year {
            query = query.filter(songs::year.le(to));
        }

        if let Some(folder_id) = music_folder_id {
            query = query.filter(songs::music_folder_id.eq(folder_id));
        }

        let results = query
            .select(SongRow::as_select())
            .order(diesel::dsl::sql::<diesel::sql_types::Integer>("RANDOM()"))
            .limit(size)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Song::from).collect())
    }

    /// Find songs by genre with pagination.
    pub fn find_by_genre(
        &self,
        genre: &str,
        count: i64,
        offset: i64,
        music_folder_id: Option<i32>,
    ) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let mut query = songs::table.into_boxed();

        query = query.filter(songs::genre.eq(genre));

        if let Some(folder_id) = music_folder_id {
            query = query.filter(songs::music_folder_id.eq(folder_id));
        }

        let results = query
            .select(SongRow::as_select())
            .order(songs::title.asc())
            .offset(offset)
            .limit(count)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Song::from).collect())
    }

    /// Find songs by IDs.
    pub fn find_by_ids(&self, song_ids: &[i32]) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = songs::table
            .filter(songs::id.eq_any(song_ids))
            .select(SongRow::as_select())
            .load(&mut conn)?;

        Ok(results.into_iter().map(Song::from).collect())
    }

    /// Find random songs by artist, excluding a specific song.
    /// Used for getSimilarSongs2 endpoint.
    pub fn find_random_by_artist(
        &self,
        artist_id: i32,
        exclude_song_id: i32,
        limit: i64,
    ) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = songs::table
            .filter(songs::artist_id.eq(artist_id))
            .filter(songs::id.ne(exclude_song_id))
            .select(SongRow::as_select())
            .order(diesel::dsl::sql::<diesel::sql_types::Integer>("RANDOM()"))
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Song::from).collect())
    }

    /// Find a song by artist name and title.
    pub fn find_by_artist_and_title(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<Option<Song>, MusicRepoError> {
        let mut conn = self.pool.get().map_err(MusicRepoError::from)?;

        let result = songs::table
            .filter(songs::artist_name.eq(artist))
            .filter(songs::title.eq(title))
            .select(SongRow::as_select())
            .first(&mut conn)
            .optional()
            .map_err(MusicRepoError::from)?;

        Ok(result.map(Song::from))
    }

    /// Find top songs by artist name, ordered by play count.
    /// Used for getTopSongs endpoint.
    pub fn find_top_by_artist_name(
        &self,
        artist_name: &str,
        limit: i64,
    ) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get().map_err(MusicRepoError::from)?;

        let results = songs::table
            .filter(songs::artist_name.eq(artist_name))
            .select(SongRow::as_select())
            .order(songs::play_count.desc())
            .limit(limit)
            .load(&mut conn)
            .map_err(MusicRepoError::from)?;

        Ok(results.into_iter().map(Song::from).collect())
    }
}
