//! Database repository for user operations.

use std::collections::HashMap;

use chrono::NaiveDateTime;
use diesel::prelude::*;
use thiserror::Error;

use crate::db::DbPool;
use crate::db::schema::{
    albums, artists, music_folders, play_queue, play_queue_songs, playlist_songs, playlists, songs,
    starred, user_ratings, users,
};
use crate::models::User;
use crate::models::music::{Album, Artist, MusicFolder, NewMusicFolder, Song};
use crate::models::user::UserRoles;

/// Errors that can occur during user repository operations.
#[derive(Debug, Error)]
pub enum UserRepoError {
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[error("Connection pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),

    #[error("User not found: {0}")]
    NotFound(String),

    #[error("Username already exists: {0}")]
    UsernameExists(String),
}

/// Database row representation for users.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[allow(clippy::struct_excessive_bools)]
pub struct UserRow {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
    pub email: Option<String>,
    pub admin_role: bool,
    pub settings_role: bool,
    pub stream_role: bool,
    pub jukebox_role: bool,
    pub download_role: bool,
    pub upload_role: bool,
    pub playlist_role: bool,
    pub cover_art_role: bool,
    pub comment_role: bool,
    pub podcast_role: bool,
    pub share_role: bool,
    pub video_conversion_role: bool,
    pub max_bit_rate: i32,
    #[allow(dead_code)]
    pub created_at: String,
    #[allow(dead_code)]
    pub updated_at: String,
    pub subsonic_password: Option<String>,
    pub api_key: Option<String>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            password_hash: row.password_hash,
            subsonic_password: row.subsonic_password,
            api_key: row.api_key,
            email: row.email,
            roles: UserRoles {
                admin_role: row.admin_role,
                settings_role: row.settings_role,
                stream_role: row.stream_role,
                jukebox_role: row.jukebox_role,
                download_role: row.download_role,
                upload_role: row.upload_role,
                playlist_role: row.playlist_role,
                cover_art_role: row.cover_art_role,
                comment_role: row.comment_role,
                podcast_role: row.podcast_role,
                share_role: row.share_role,
                video_conversion_role: row.video_conversion_role,
            },
            max_bit_rate: row.max_bit_rate,
        }
    }
}

/// Data for inserting a new user.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = users)]
#[allow(clippy::struct_excessive_bools)]
pub struct NewUser<'a> {
    pub username: &'a str,
    pub password_hash: &'a str,
    pub subsonic_password: Option<&'a str>,
    pub email: Option<&'a str>,
    pub admin_role: bool,
    pub settings_role: bool,
    pub stream_role: bool,
    pub jukebox_role: bool,
    pub download_role: bool,
    pub upload_role: bool,
    pub playlist_role: bool,
    pub cover_art_role: bool,
    pub comment_role: bool,
    pub podcast_role: bool,
    pub share_role: bool,
    pub video_conversion_role: bool,
    pub max_bit_rate: i32,
}

impl<'a> NewUser<'a> {
    /// Create a new admin user.
    #[must_use]
    pub const fn admin(
        username: &'a str,
        password_hash: &'a str,
        subsonic_password: &'a str,
    ) -> Self {
        Self {
            username,
            password_hash,
            subsonic_password: Some(subsonic_password),
            email: None,
            admin_role: true,
            settings_role: true,
            stream_role: true,
            jukebox_role: true,
            download_role: true,
            upload_role: true,
            playlist_role: true,
            cover_art_role: true,
            comment_role: true,
            podcast_role: true,
            share_role: true,
            video_conversion_role: true,
            max_bit_rate: 0,
        }
    }

    /// Create a new regular user with default permissions.
    #[must_use]
    pub const fn regular(
        username: &'a str,
        password_hash: &'a str,
        subsonic_password: &'a str,
    ) -> Self {
        Self {
            username,
            password_hash,
            subsonic_password: Some(subsonic_password),
            email: None,
            admin_role: false,
            settings_role: true,
            stream_role: true,
            jukebox_role: false,
            download_role: true,
            upload_role: false,
            playlist_role: true,
            cover_art_role: true,
            comment_role: false,
            podcast_role: false,
            share_role: false,
            video_conversion_role: false,
            max_bit_rate: 0,
        }
    }
}

/// Repository for user database operations.
#[derive(Clone, Debug)]
pub struct UserRepository {
    pool: DbPool,
}

impl UserRepository {
    /// Create a new user repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Find a user by username.
    pub fn find_by_username(&self, username: &str) -> Result<Option<User>, UserRepoError> {
        let mut conn = self.pool.get()?;

        let result = users::table
            .filter(users::username.eq(username))
            .select(UserRow::as_select())
            .first(&mut conn)
            .optional()?;

        Ok(result.map(User::from))
    }

    /// Find a user by ID.
    pub fn find_by_id(&self, user_id: i32) -> Result<Option<User>, UserRepoError> {
        let mut conn = self.pool.get()?;

        let result = users::table
            .filter(users::id.eq(user_id))
            .select(UserRow::as_select())
            .first(&mut conn)
            .optional()?;

        Ok(result.map(User::from))
    }

    /// Get all users.
    pub fn find_all(&self) -> Result<Vec<User>, UserRepoError> {
        let mut conn = self.pool.get()?;

        let results = users::table.select(UserRow::as_select()).load(&mut conn)?;

        Ok(results.into_iter().map(User::from).collect())
    }

    /// Create a new user.
    pub fn create(&self, new_user: &NewUser) -> Result<User, UserRepoError> {
        let mut conn = self.pool.get()?;

        // Check if username already exists
        let existing = users::table
            .filter(users::username.eq(new_user.username))
            .count()
            .get_result::<i64>(&mut conn)?;

        if existing > 0 {
            return Err(UserRepoError::UsernameExists(new_user.username.to_string()));
        }

        diesel::insert_into(users::table)
            .values(new_user)
            .execute(&mut conn)?;

        // Fetch the created user
        let user = users::table
            .filter(users::username.eq(new_user.username))
            .select(UserRow::as_select())
            .first(&mut conn)?;

        Ok(User::from(user))
    }

    /// Delete a user by ID.
    pub fn delete(&self, user_id: i32) -> Result<bool, UserRepoError> {
        let mut conn = self.pool.get()?;

        let deleted =
            diesel::delete(users::table.filter(users::id.eq(user_id))).execute(&mut conn)?;

        Ok(deleted > 0)
    }

    /// Update a user's password.
    pub fn update_password(
        &self,
        user_id: i32,
        password_hash: &str,
    ) -> Result<bool, UserRepoError> {
        let mut conn = self.pool.get()?;

        let updated = diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::password_hash.eq(password_hash))
            .execute(&mut conn)?;

        Ok(updated > 0)
    }

    /// Check if any users exist in the database.
    pub fn has_users(&self) -> Result<bool, UserRepoError> {
        let mut conn = self.pool.get()?;

        let count = users::table.count().get_result::<i64>(&mut conn)?;

        Ok(count > 0)
    }

    /// Find a user by API key.
    ///
    /// Note: This uses a database query with an index lookup, which may be
    /// vulnerable to timing attacks. For a personal music server this is
    /// acceptable given the high entropy of API keys (128 bits). For higher
    /// security requirements, consider storing and comparing API key hashes.
    pub fn find_by_api_key(&self, api_key: &str) -> Result<Option<User>, UserRepoError> {
        let mut conn = self.pool.get()?;

        let result = users::table
            .filter(users::api_key.eq(api_key))
            .select(UserRow::as_select())
            .first(&mut conn)
            .optional()?;

        Ok(result.map(User::from))
    }

    /// Set or update a user's API key.
    pub fn set_api_key(&self, user_id: i32, api_key: Option<&str>) -> Result<bool, UserRepoError> {
        let mut conn = self.pool.get()?;

        let updated = diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::api_key.eq(api_key))
            .execute(&mut conn)?;

        Ok(updated > 0)
    }

    /// Generate a new API key for a user.
    /// Returns the generated API key.
    pub fn generate_api_key(&self, user_id: i32) -> Result<String, UserRepoError> {
        use rand_core::{OsRng, RngCore};

        // Generate a random 32-byte key and encode as hex (64 characters)
        let mut key_bytes = [0u8; 32];
        OsRng.fill_bytes(&mut key_bytes);
        let api_key = hex::encode(key_bytes);

        self.set_api_key(user_id, Some(&api_key))?;
        Ok(api_key)
    }

    /// Revoke a user's API key.
    pub fn revoke_api_key(&self, user_id: i32) -> Result<bool, UserRepoError> {
        self.set_api_key(user_id, None)
    }

    /// Update a user's subsonic password (used for token auth).
    pub fn update_subsonic_password(
        &self,
        user_id: i32,
        subsonic_password: &str,
    ) -> Result<bool, UserRepoError> {
        let mut conn = self.pool.get()?;

        let updated = diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::subsonic_password.eq(Some(subsonic_password)))
            .execute(&mut conn)?;

        Ok(updated > 0)
    }

    /// Update a user's profile and roles.
    pub fn update_user(&self, update: &UserUpdate) -> Result<bool, UserRepoError> {
        let mut conn = self.pool.get()?;

        // Find the user first
        let user = users::table
            .filter(users::username.eq(&update.username))
            .select(UserRow::as_select())
            .first(&mut conn)
            .optional()?
            .ok_or_else(|| UserRepoError::NotFound(update.username.clone()))?;

        // Build the update - we update all provided fields
        let updated = diesel::update(users::table.filter(users::id.eq(user.id)))
            .set((
                update
                    .email
                    .as_ref()
                    .map(|e| users::email.eq(Some(e.as_str()))),
                update.admin_role.map(|v| users::admin_role.eq(v)),
                update.settings_role.map(|v| users::settings_role.eq(v)),
                update.stream_role.map(|v| users::stream_role.eq(v)),
                update.jukebox_role.map(|v| users::jukebox_role.eq(v)),
                update.download_role.map(|v| users::download_role.eq(v)),
                update.upload_role.map(|v| users::upload_role.eq(v)),
                update.playlist_role.map(|v| users::playlist_role.eq(v)),
                update.cover_art_role.map(|v| users::cover_art_role.eq(v)),
                update.comment_role.map(|v| users::comment_role.eq(v)),
                update.podcast_role.map(|v| users::podcast_role.eq(v)),
                update.share_role.map(|v| users::share_role.eq(v)),
                update
                    .video_conversion_role
                    .map(|v| users::video_conversion_role.eq(v)),
                update.max_bit_rate.map(|v| users::max_bit_rate.eq(v)),
            ))
            .execute(&mut conn)?;

        Ok(updated > 0)
    }
}

/// Data for updating an existing user.
#[derive(Debug, Clone, Default)]
pub struct UserUpdate {
    pub username: String,
    pub email: Option<String>,
    pub admin_role: Option<bool>,
    pub settings_role: Option<bool>,
    pub stream_role: Option<bool>,
    pub jukebox_role: Option<bool>,
    pub download_role: Option<bool>,
    pub upload_role: Option<bool>,
    pub playlist_role: Option<bool>,
    pub cover_art_role: Option<bool>,
    pub comment_role: Option<bool>,
    pub podcast_role: Option<bool>,
    pub share_role: Option<bool>,
    pub video_conversion_role: Option<bool>,
    pub max_bit_rate: Option<i32>,
}

// ============================================================================
// Music Library Repositories
// ============================================================================

/// Errors that can occur during music library repository operations.
#[derive(Debug, Error)]
pub enum MusicRepoError {
    #[error("Database error: {0}")]
    Database(#[from] diesel::result::Error),

    #[error("Connection pool error: {0}")]
    Pool(#[from] diesel::r2d2::PoolError),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Already exists: {0}")]
    AlreadyExists(String),
}

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
            return Err(MusicRepoError::AlreadyExists(new_folder.path.clone()));
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

    /// Get all artists ordered by name.
    pub fn find_all(&self) -> Result<Vec<Artist>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = artists::table
            .select(ArtistRow::as_select())
            .order(artists::name.asc())
            .load(&mut conn)?;

        Ok(results.into_iter().map(Artist::from).collect())
    }

    /// Find an artist by ID.
    pub fn find_by_id(&self, artist_id: i32) -> Result<Option<Artist>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = artists::table
            .filter(artists::id.eq(artist_id))
            .select(ArtistRow::as_select())
            .first(&mut conn)
            .optional()?;

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

    /// Find top songs by artist name, ordered by play count.
    /// Used for getTopSongs endpoint.
    pub fn find_top_by_artist_name(
        &self,
        artist_name: &str,
        limit: i64,
    ) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results = songs::table
            .filter(songs::artist_name.eq(artist_name))
            .select(SongRow::as_select())
            .order(songs::play_count.desc())
            .limit(limit)
            .load(&mut conn)?;

        Ok(results.into_iter().map(Song::from).collect())
    }
}

// ============================================================================
// Starred Repository
// ============================================================================

/// Database row representation for starred items.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = starred)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct StarredRow {
    pub id: i32,
    pub user_id: i32,
    pub artist_id: Option<i32>,
    pub album_id: Option<i32>,
    pub song_id: Option<i32>,
    pub starred_at: NaiveDateTime,
}

/// Data for inserting a new starred item.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = starred)]
pub struct NewStarred {
    pub user_id: i32,
    pub artist_id: Option<i32>,
    pub album_id: Option<i32>,
    pub song_id: Option<i32>,
}

/// Repository for starred items database operations.
#[derive(Clone, Debug)]
pub struct StarredRepository {
    pool: DbPool,
}

impl StarredRepository {
    /// Create a new starred repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    // ========================================================================
    // Star operations
    // ========================================================================

    /// Star an artist for a user.
    pub fn star_artist(&self, user_id: i32, artist_id: i32) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Use INSERT OR IGNORE to handle race conditions atomically.
        // If the entry already exists, this is a no-op.
        let new_starred = NewStarred {
            user_id,
            artist_id: Some(artist_id),
            album_id: None,
            song_id: None,
        };

        diesel::insert_or_ignore_into(starred::table)
            .values(&new_starred)
            .execute(&mut conn)?;

        Ok(())
    }

    /// Star an album for a user.
    pub fn star_album(&self, user_id: i32, album_id: i32) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        let new_starred = NewStarred {
            user_id,
            artist_id: None,
            album_id: Some(album_id),
            song_id: None,
        };

        diesel::insert_or_ignore_into(starred::table)
            .values(&new_starred)
            .execute(&mut conn)?;

        Ok(())
    }

    /// Star a song for a user.
    pub fn star_song(&self, user_id: i32, song_id: i32) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        let new_starred = NewStarred {
            user_id,
            artist_id: None,
            album_id: None,
            song_id: Some(song_id),
        };

        diesel::insert_or_ignore_into(starred::table)
            .values(&new_starred)
            .execute(&mut conn)?;

        Ok(())
    }

    // ========================================================================
    // Unstar operations
    // ========================================================================

    /// Unstar an artist for a user.
    pub fn unstar_artist(&self, user_id: i32, artist_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let deleted = diesel::delete(
            starred::table
                .filter(starred::user_id.eq(user_id))
                .filter(starred::artist_id.eq(artist_id)),
        )
        .execute(&mut conn)?;

        Ok(deleted > 0)
    }

    /// Unstar an album for a user.
    pub fn unstar_album(&self, user_id: i32, album_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let deleted = diesel::delete(
            starred::table
                .filter(starred::user_id.eq(user_id))
                .filter(starred::album_id.eq(album_id)),
        )
        .execute(&mut conn)?;

        Ok(deleted > 0)
    }

    /// Unstar a song for a user.
    pub fn unstar_song(&self, user_id: i32, song_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let deleted = diesel::delete(
            starred::table
                .filter(starred::user_id.eq(user_id))
                .filter(starred::song_id.eq(song_id)),
        )
        .execute(&mut conn)?;

        Ok(deleted > 0)
    }

    // ========================================================================
    // Query operations
    // ========================================================================

    /// Get all starred artists for a user with their starred timestamp.
    /// Returns (Artist, `starred_at`).
    pub fn get_starred_artists(
        &self,
        user_id: i32,
    ) -> Result<Vec<(Artist, NaiveDateTime)>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results: Vec<(StarredRow, ArtistRow)> = starred::table
            .inner_join(artists::table.on(starred::artist_id.eq(artists::id.nullable())))
            .filter(starred::user_id.eq(user_id))
            .filter(starred::artist_id.is_not_null())
            .select((StarredRow::as_select(), ArtistRow::as_select()))
            .order(starred::starred_at.desc())
            .load(&mut conn)?;

        Ok(results
            .into_iter()
            .map(|(s, a)| (Artist::from(a), s.starred_at))
            .collect())
    }

    /// Get all starred albums for a user with their starred timestamp.
    /// Returns (Album, `starred_at`).
    pub fn get_starred_albums(
        &self,
        user_id: i32,
    ) -> Result<Vec<(Album, NaiveDateTime)>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results: Vec<(StarredRow, AlbumRow)> = starred::table
            .inner_join(albums::table.on(starred::album_id.eq(albums::id.nullable())))
            .filter(starred::user_id.eq(user_id))
            .filter(starred::album_id.is_not_null())
            .select((StarredRow::as_select(), AlbumRow::as_select()))
            .order(starred::starred_at.desc())
            .load(&mut conn)?;

        Ok(results
            .into_iter()
            .map(|(s, a)| (Album::from(a), s.starred_at))
            .collect())
    }

    /// Get all starred songs for a user with their starred timestamp.
    /// Returns (Song, `starred_at`).
    pub fn get_starred_songs(
        &self,
        user_id: i32,
    ) -> Result<Vec<(Song, NaiveDateTime)>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results: Vec<(StarredRow, SongRow)> = starred::table
            .inner_join(songs::table.on(starred::song_id.eq(songs::id.nullable())))
            .filter(starred::user_id.eq(user_id))
            .filter(starred::song_id.is_not_null())
            .select((StarredRow::as_select(), SongRow::as_select()))
            .order(starred::starred_at.desc())
            .load(&mut conn)?;

        Ok(results
            .into_iter()
            .map(|(s, song)| (Song::from(song), s.starred_at))
            .collect())
    }

    /// Check if an artist is starred by a user.
    pub fn is_artist_starred(&self, user_id: i32, artist_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let count = starred::table
            .filter(starred::user_id.eq(user_id))
            .filter(starred::artist_id.eq(artist_id))
            .count()
            .get_result::<i64>(&mut conn)?;

        Ok(count > 0)
    }

    /// Check if an album is starred by a user.
    pub fn is_album_starred(&self, user_id: i32, album_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let count = starred::table
            .filter(starred::user_id.eq(user_id))
            .filter(starred::album_id.eq(album_id))
            .count()
            .get_result::<i64>(&mut conn)?;

        Ok(count > 0)
    }

    /// Check if a song is starred by a user.
    pub fn is_song_starred(&self, user_id: i32, song_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let count = starred::table
            .filter(starred::user_id.eq(user_id))
            .filter(starred::song_id.eq(song_id))
            .count()
            .get_result::<i64>(&mut conn)?;

        Ok(count > 0)
    }

    /// Get the `starred_at` timestamp for an artist.
    pub fn get_starred_at_for_artist(
        &self,
        user_id: i32,
        artist_id: i32,
    ) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = starred::table
            .filter(starred::user_id.eq(user_id))
            .filter(starred::artist_id.eq(artist_id))
            .select(starred::starred_at)
            .first(&mut conn)
            .optional()?;

        Ok(result)
    }

    /// Get the `starred_at` timestamp for an album.
    pub fn get_starred_at_for_album(
        &self,
        user_id: i32,
        album_id: i32,
    ) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = starred::table
            .filter(starred::user_id.eq(user_id))
            .filter(starred::album_id.eq(album_id))
            .select(starred::starred_at)
            .first(&mut conn)
            .optional()?;

        Ok(result)
    }

    /// Get the `starred_at` timestamp for a song.
    pub fn get_starred_at_for_song(
        &self,
        user_id: i32,
        song_id: i32,
    ) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = starred::table
            .filter(starred::user_id.eq(user_id))
            .filter(starred::song_id.eq(song_id))
            .select(starred::starred_at)
            .first(&mut conn)
            .optional()?;

        Ok(result)
    }

    /// Get starred albums for a user with pagination, ordered by `starred_at` descending.
    /// Returns albums with their `starred_at` timestamp.
    pub fn get_starred_albums_paginated(
        &self,
        user_id: i32,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<(Album, NaiveDateTime)>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results: Vec<(StarredRow, AlbumRow)> = starred::table
            .inner_join(albums::table.on(starred::album_id.eq(albums::id.nullable())))
            .filter(starred::user_id.eq(user_id))
            .filter(starred::album_id.is_not_null())
            .select((StarredRow::as_select(), AlbumRow::as_select()))
            .order(starred::starred_at.desc())
            .offset(offset)
            .limit(limit)
            .load(&mut conn)?;

        Ok(results
            .into_iter()
            .map(|(s, a)| (Album::from(a), s.starred_at))
            .collect())
    }

    // ========================================================================
    // Batch query operations (to fix N+1 queries)
    // ========================================================================

    /// Get `starred_at` timestamps for multiple songs in a single query.
    /// Returns a `HashMap` mapping `song_id` to `starred_at`.
    pub fn get_starred_at_for_songs_batch(
        &self,
        user_id: i32,
        song_ids: &[i32],
    ) -> Result<std::collections::HashMap<i32, NaiveDateTime>, MusicRepoError> {
        if song_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = self.pool.get()?;

        let results: Vec<(i32, NaiveDateTime)> = starred::table
            .filter(starred::user_id.eq(user_id))
            .filter(starred::song_id.eq_any(song_ids))
            .select((starred::song_id.assume_not_null(), starred::starred_at))
            .load(&mut conn)?;

        Ok(results.into_iter().collect())
    }

    /// Get `starred_at` timestamps for multiple albums in a single query.
    /// Returns a `HashMap` mapping `album_id` to `starred_at`.
    pub fn get_starred_at_for_albums_batch(
        &self,
        user_id: i32,
        album_ids: &[i32],
    ) -> Result<std::collections::HashMap<i32, NaiveDateTime>, MusicRepoError> {
        if album_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = self.pool.get()?;

        let results: Vec<(i32, NaiveDateTime)> = starred::table
            .filter(starred::user_id.eq(user_id))
            .filter(starred::album_id.eq_any(album_ids))
            .select((starred::album_id.assume_not_null(), starred::starred_at))
            .load(&mut conn)?;

        Ok(results.into_iter().collect())
    }

    /// Get `starred_at` timestamps for multiple artists in a single query.
    /// Returns a `HashMap` mapping `artist_id` to `starred_at`.
    pub fn get_starred_at_for_artists_batch(
        &self,
        user_id: i32,
        artist_ids: &[i32],
    ) -> Result<std::collections::HashMap<i32, NaiveDateTime>, MusicRepoError> {
        if artist_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut conn = self.pool.get()?;

        let results: Vec<(i32, NaiveDateTime)> = starred::table
            .filter(starred::user_id.eq(user_id))
            .filter(starred::artist_id.eq_any(artist_ids))
            .select((starred::artist_id.assume_not_null(), starred::starred_at))
            .load(&mut conn)?;

        Ok(results.into_iter().collect())
    }
}

// ============================================================================
// Now Playing Repository
// ============================================================================

use crate::db::schema::now_playing;

/// Database row representation for now playing.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = now_playing)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct NowPlayingRow {
    pub id: i32,
    pub user_id: i32,
    pub song_id: i32,
    pub player_id: Option<String>,
    pub started_at: NaiveDateTime,
    pub minutes_ago: i32,
}

/// Data for inserting a now playing entry.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = now_playing)]
pub struct NewNowPlaying {
    pub user_id: i32,
    pub song_id: i32,
    pub player_id: Option<String>,
}

/// Now playing entry with song and user info.
#[derive(Debug, Clone)]
pub struct NowPlayingEntry {
    pub song: Song,
    pub username: String,
    pub player_id: Option<String>,
    pub minutes_ago: i32,
}

/// Repository for now playing database operations.
#[derive(Clone, Debug)]
pub struct NowPlayingRepository {
    pool: DbPool,
}

impl NowPlayingRepository {
    /// Create a new now playing repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Set a song as now playing for a user.
    /// Replaces any existing now playing entry for the user.
    pub fn set_now_playing(
        &self,
        user_id: i32,
        song_id: i32,
        player_id: Option<&str>,
    ) -> Result<(), MusicRepoError> {
        use diesel::upsert::excluded;

        let mut conn = self.pool.get()?;

        // Use upsert to atomically replace any existing entry for this user.
        // This avoids race conditions that can occur with DELETE + INSERT.
        let new_entry = NewNowPlaying {
            user_id,
            song_id,
            player_id: player_id.map(std::string::ToString::to_string),
        };

        diesel::insert_into(now_playing::table)
            .values(&new_entry)
            .on_conflict(now_playing::user_id)
            .do_update()
            .set((
                now_playing::song_id.eq(excluded(now_playing::song_id)),
                now_playing::player_id.eq(excluded(now_playing::player_id)),
                now_playing::started_at.eq(diesel::dsl::now),
                now_playing::minutes_ago.eq(0),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    /// Clear the now playing entry for a user.
    pub fn clear_now_playing(&self, user_id: i32) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        diesel::delete(now_playing::table.filter(now_playing::user_id.eq(user_id)))
            .execute(&mut conn)?;

        Ok(())
    }

    /// Get all currently playing songs.
    /// Returns entries with song and user info, ordered by most recent.
    pub fn get_all_now_playing(&self) -> Result<Vec<NowPlayingEntry>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results: Vec<(NowPlayingRow, SongRow, UserRow)> = now_playing::table
            .inner_join(songs::table.on(now_playing::song_id.eq(songs::id)))
            .inner_join(users::table.on(now_playing::user_id.eq(users::id)))
            .select((
                NowPlayingRow::as_select(),
                SongRow::as_select(),
                UserRow::as_select(),
            ))
            .order(now_playing::started_at.desc())
            .load(&mut conn)?;

        let now = chrono::Utc::now().naive_utc();

        Ok(results
            .into_iter()
            .map(|(np, song, user)| {
                let minutes_ago = i32::try_from((now - np.started_at).num_minutes()).unwrap_or(0);
                NowPlayingEntry {
                    song: Song::from(song),
                    username: user.username,
                    player_id: np.player_id,
                    minutes_ago,
                }
            })
            .collect())
    }
}

// ============================================================================
// Scrobble Repository
// ============================================================================

use crate::db::schema::scrobbles;

/// Database row representation for scrobbles.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = scrobbles)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct ScrobbleRow {
    pub id: i32,
    pub user_id: i32,
    pub song_id: i32,
    pub played_at: NaiveDateTime,
    pub submission: bool,
}

/// Data for inserting a scrobble.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = scrobbles)]
pub struct NewScrobble {
    pub user_id: i32,
    pub song_id: i32,
    pub played_at: NaiveDateTime,
    pub submission: bool,
}

/// Repository for scrobble database operations.
#[derive(Clone, Debug)]
pub struct ScrobbleRepository {
    pool: DbPool,
}

impl ScrobbleRepository {
    /// Create a new scrobble repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Record a scrobble (song play).
    pub fn scrobble(
        &self,
        user_id: i32,
        song_id: i32,
        time: Option<i64>,
        submission: bool,
    ) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Determine the played_at timestamp
        let played_at = time.map_or_else(
            || chrono::Utc::now().naive_utc(),
            |timestamp_ms| {
                // Convert milliseconds since epoch to NaiveDateTime
                chrono::DateTime::from_timestamp_millis(timestamp_ms)
                    .map_or_else(|| chrono::Utc::now().naive_utc(), |dt| dt.naive_utc())
            },
        );

        let new_scrobble = NewScrobble {
            user_id,
            song_id,
            played_at,
            submission,
        };

        diesel::insert_into(scrobbles::table)
            .values(&new_scrobble)
            .execute(&mut conn)?;

        // If this is a submission (full play), increment the song's play count
        if submission {
            diesel::update(songs::table.filter(songs::id.eq(song_id)))
                .set(songs::play_count.eq(songs::play_count + 1))
                .execute(&mut conn)?;

            // Also try to increment the album's play count
            let album_id: Option<i32> = songs::table
                .filter(songs::id.eq(song_id))
                .select(songs::album_id)
                .first(&mut conn)
                .optional()?
                .flatten();

            if let Some(aid) = album_id {
                diesel::update(albums::table.filter(albums::id.eq(aid)))
                    .set(albums::play_count.eq(albums::play_count + 1))
                    .execute(&mut conn)?;
            }
        }

        Ok(())
    }

    /// Get recent scrobbles for a user.
    pub fn get_recent_scrobbles(
        &self,
        user_id: i32,
        limit: i64,
    ) -> Result<Vec<(Song, NaiveDateTime)>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results: Vec<(ScrobbleRow, SongRow)> = scrobbles::table
            .inner_join(songs::table.on(scrobbles::song_id.eq(songs::id)))
            .filter(scrobbles::user_id.eq(user_id))
            .filter(scrobbles::submission.eq(true))
            .select((ScrobbleRow::as_select(), SongRow::as_select()))
            .order(scrobbles::played_at.desc())
            .limit(limit)
            .load(&mut conn)?;

        Ok(results
            .into_iter()
            .map(|(scrobble, song)| (Song::from(song), scrobble.played_at))
            .collect())
    }
}

// ============================================================================
// Rating Repository
// ============================================================================

/// Database row representation for user ratings.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = user_ratings)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct UserRatingRow {
    pub id: i32,
    pub user_id: i32,
    pub song_id: Option<i32>,
    pub album_id: Option<i32>,
    pub artist_id: Option<i32>,
    pub rating: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Data for inserting a user rating.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = user_ratings)]
pub struct NewUserRating {
    pub user_id: i32,
    pub song_id: Option<i32>,
    pub album_id: Option<i32>,
    pub artist_id: Option<i32>,
    pub rating: i32,
}

/// Repository for user rating database operations.
#[derive(Clone, Debug)]
pub struct RatingRepository {
    pool: DbPool,
}

impl RatingRepository {
    /// Create a new rating repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Set rating for a song. Rating of 0 removes the rating.
    pub fn set_song_rating(
        &self,
        user_id: i32,
        song_id: i32,
        rating: i32,
    ) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        if rating == 0 {
            // Remove rating
            diesel::delete(
                user_ratings::table
                    .filter(user_ratings::user_id.eq(user_id))
                    .filter(user_ratings::song_id.eq(song_id)),
            )
            .execute(&mut conn)?;
        } else {
            // Use upsert to atomically insert or update the rating
            diesel::sql_query(
                "INSERT INTO user_ratings (user_id, song_id, rating, updated_at)
                 VALUES (?, ?, ?, CURRENT_TIMESTAMP)
                 ON CONFLICT (user_id, song_id) WHERE song_id IS NOT NULL
                 DO UPDATE SET rating = excluded.rating, updated_at = CURRENT_TIMESTAMP",
            )
            .bind::<diesel::sql_types::Integer, _>(user_id)
            .bind::<diesel::sql_types::Integer, _>(song_id)
            .bind::<diesel::sql_types::Integer, _>(rating)
            .execute(&mut conn)?;
        }

        Ok(())
    }

    /// Set rating for an album. Rating of 0 removes the rating.
    pub fn set_album_rating(
        &self,
        user_id: i32,
        album_id: i32,
        rating: i32,
    ) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        if rating == 0 {
            diesel::delete(
                user_ratings::table
                    .filter(user_ratings::user_id.eq(user_id))
                    .filter(user_ratings::album_id.eq(album_id)),
            )
            .execute(&mut conn)?;
        } else {
            diesel::sql_query(
                "INSERT INTO user_ratings (user_id, album_id, rating, updated_at)
                 VALUES (?, ?, ?, CURRENT_TIMESTAMP)
                 ON CONFLICT (user_id, album_id) WHERE album_id IS NOT NULL
                 DO UPDATE SET rating = excluded.rating, updated_at = CURRENT_TIMESTAMP",
            )
            .bind::<diesel::sql_types::Integer, _>(user_id)
            .bind::<diesel::sql_types::Integer, _>(album_id)
            .bind::<diesel::sql_types::Integer, _>(rating)
            .execute(&mut conn)?;
        }

        Ok(())
    }

    /// Set rating for an artist. Rating of 0 removes the rating.
    pub fn set_artist_rating(
        &self,
        user_id: i32,
        artist_id: i32,
        rating: i32,
    ) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        if rating == 0 {
            diesel::delete(
                user_ratings::table
                    .filter(user_ratings::user_id.eq(user_id))
                    .filter(user_ratings::artist_id.eq(artist_id)),
            )
            .execute(&mut conn)?;
        } else {
            diesel::sql_query(
                "INSERT INTO user_ratings (user_id, artist_id, rating, updated_at)
                 VALUES (?, ?, ?, CURRENT_TIMESTAMP)
                 ON CONFLICT (user_id, artist_id) WHERE artist_id IS NOT NULL
                 DO UPDATE SET rating = excluded.rating, updated_at = CURRENT_TIMESTAMP",
            )
            .bind::<diesel::sql_types::Integer, _>(user_id)
            .bind::<diesel::sql_types::Integer, _>(artist_id)
            .bind::<diesel::sql_types::Integer, _>(rating)
            .execute(&mut conn)?;
        }

        Ok(())
    }

    /// Get rating for a song.
    pub fn get_song_rating(
        &self,
        user_id: i32,
        song_id: i32,
    ) -> Result<Option<i32>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = user_ratings::table
            .filter(user_ratings::user_id.eq(user_id))
            .filter(user_ratings::song_id.eq(song_id))
            .select(user_ratings::rating)
            .first(&mut conn)
            .optional()?;

        Ok(result)
    }

    /// Get rating for an album.
    pub fn get_album_rating(
        &self,
        user_id: i32,
        album_id: i32,
    ) -> Result<Option<i32>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = user_ratings::table
            .filter(user_ratings::user_id.eq(user_id))
            .filter(user_ratings::album_id.eq(album_id))
            .select(user_ratings::rating)
            .first(&mut conn)
            .optional()?;

        Ok(result)
    }

    /// Get rating for an artist.
    pub fn get_artist_rating(
        &self,
        user_id: i32,
        artist_id: i32,
    ) -> Result<Option<i32>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result = user_ratings::table
            .filter(user_ratings::user_id.eq(user_id))
            .filter(user_ratings::artist_id.eq(artist_id))
            .select(user_ratings::rating)
            .first(&mut conn)
            .optional()?;

        Ok(result)
    }

    /// Get highest rated albums for a user with pagination.
    /// Returns album IDs ordered by rating descending, then by album name.
    pub fn get_highest_rated_album_ids(
        &self,
        user_id: i32,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<i32>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results: Vec<i32> = user_ratings::table
            .filter(user_ratings::user_id.eq(user_id))
            .filter(user_ratings::album_id.is_not_null())
            .filter(user_ratings::rating.gt(0))
            .select(user_ratings::album_id)
            .order(user_ratings::rating.desc())
            .offset(offset)
            .limit(limit)
            .load::<Option<i32>>(&mut conn)?
            .into_iter()
            .flatten()
            .collect();

        Ok(results)
    }
}

// ============================================================================
// Playlist Repository
// ============================================================================

/// Database row representation for playlists.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = playlists)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PlaylistRow {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub comment: Option<String>,
    pub public: bool,
    pub song_count: i32,
    pub duration: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Data for inserting a new playlist.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = playlists)]
pub struct NewPlaylist<'a> {
    pub user_id: i32,
    pub name: &'a str,
    pub comment: Option<&'a str>,
    pub public: bool,
}

/// Database row representation for playlist songs.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = playlist_songs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PlaylistSongRow {
    pub id: i32,
    pub playlist_id: i32,
    pub song_id: i32,
    pub position: i32,
    pub created_at: NaiveDateTime,
}

/// Data for inserting a playlist song.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = playlist_songs)]
pub struct NewPlaylistSong {
    pub playlist_id: i32,
    pub song_id: i32,
    pub position: i32,
}

/// Playlist with owner info.
#[derive(Debug, Clone)]
pub struct Playlist {
    pub id: i32,
    pub name: String,
    pub comment: Option<String>,
    pub owner: String,
    pub public: bool,
    pub song_count: i32,
    pub duration: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Repository for playlist database operations.
#[derive(Clone, Debug)]
pub struct PlaylistRepository {
    pool: DbPool,
}

impl PlaylistRepository {
    /// Create a new playlist repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get all playlists for a user (including public playlists from others).
    pub fn get_playlists(
        &self,
        user_id: i32,
        _username: &str,
    ) -> Result<Vec<Playlist>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Get playlists owned by user or public playlists
        let results: Vec<(PlaylistRow, UserRow)> = playlists::table
            .inner_join(users::table.on(playlists::user_id.eq(users::id)))
            .filter(
                playlists::user_id
                    .eq(user_id)
                    .or(playlists::public.eq(true)),
            )
            .select((PlaylistRow::as_select(), UserRow::as_select()))
            .order(playlists::name.asc())
            .load(&mut conn)?;

        Ok(results
            .into_iter()
            .map(|(p, u)| Playlist {
                id: p.id,
                name: p.name,
                comment: p.comment,
                owner: u.username,
                public: p.public,
                song_count: p.song_count,
                duration: p.duration,
                created_at: p.created_at,
                updated_at: p.updated_at,
            })
            .collect())
    }

    /// Get a playlist by ID.
    pub fn get_playlist(&self, playlist_id: i32) -> Result<Option<Playlist>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let result: Option<(PlaylistRow, UserRow)> = playlists::table
            .inner_join(users::table.on(playlists::user_id.eq(users::id)))
            .filter(playlists::id.eq(playlist_id))
            .select((PlaylistRow::as_select(), UserRow::as_select()))
            .first(&mut conn)
            .optional()?;

        Ok(result.map(|(p, u)| Playlist {
            id: p.id,
            name: p.name,
            comment: p.comment,
            owner: u.username,
            public: p.public,
            song_count: p.song_count,
            duration: p.duration,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }))
    }

    /// Get songs in a playlist, ordered by position.
    pub fn get_playlist_songs(&self, playlist_id: i32) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results: Vec<(PlaylistSongRow, SongRow)> = playlist_songs::table
            .inner_join(songs::table.on(playlist_songs::song_id.eq(songs::id)))
            .filter(playlist_songs::playlist_id.eq(playlist_id))
            .select((PlaylistSongRow::as_select(), SongRow::as_select()))
            .order(playlist_songs::position.asc())
            .load(&mut conn)?;

        Ok(results.into_iter().map(|(_, s)| Song::from(s)).collect())
    }

    /// Get cover art IDs for multiple playlists in a single query.
    /// Returns a map of `playlist_id` -> `cover_art` (from the first song in each playlist).
    pub fn get_playlist_cover_arts_batch(
        &self,
        playlist_ids: &[i32],
    ) -> Result<std::collections::HashMap<i32, String>, MusicRepoError> {
        if playlist_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let mut conn = self.pool.get()?;

        // Get the first song (position 0) for each playlist and join to get cover_art
        let results: Vec<(i32, Option<String>)> = playlist_songs::table
            .inner_join(songs::table.on(playlist_songs::song_id.eq(songs::id)))
            .filter(playlist_songs::playlist_id.eq_any(playlist_ids))
            .filter(playlist_songs::position.eq(0))
            .select((playlist_songs::playlist_id, songs::cover_art))
            .load(&mut conn)?;

        Ok(results
            .into_iter()
            .filter_map(|(pid, cover)| cover.map(|c| (pid, c)))
            .collect())
    }

    /// Create a new playlist.
    pub fn create_playlist(
        &self,
        user_id: i32,
        name: &str,
        comment: Option<&str>,
        song_ids: &[i32],
    ) -> Result<Playlist, MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Insert the playlist
        let new_playlist = NewPlaylist {
            user_id,
            name,
            comment,
            public: false,
        };

        diesel::insert_into(playlists::table)
            .values(&new_playlist)
            .execute(&mut conn)?;

        // Get the created playlist ID
        let playlist_id: i32 = playlists::table
            .filter(playlists::user_id.eq(user_id))
            .filter(playlists::name.eq(name))
            .order(playlists::created_at.desc())
            .select(playlists::id)
            .first(&mut conn)?;

        // Add songs to playlist
        if !song_ids.is_empty() {
            let mut total_duration = 0i32;

            for (position, song_id) in song_ids.iter().enumerate() {
                // Get song duration
                if let Some(duration) = songs::table
                    .filter(songs::id.eq(song_id))
                    .select(songs::duration)
                    .first::<i32>(&mut conn)
                    .optional()?
                {
                    total_duration += duration;

                    let new_song = NewPlaylistSong {
                        playlist_id,
                        song_id: *song_id,
                        position: i32::try_from(position).unwrap_or(0),
                    };

                    diesel::insert_into(playlist_songs::table)
                        .values(&new_song)
                        .execute(&mut conn)?;
                }
            }

            // Update playlist stats
            diesel::update(playlists::table.filter(playlists::id.eq(playlist_id)))
                .set((
                    playlists::song_count.eq(i32::try_from(song_ids.len()).unwrap_or(0)),
                    playlists::duration.eq(total_duration),
                ))
                .execute(&mut conn)?;
        }

        // Return the created playlist
        self.get_playlist(playlist_id)?
            .ok_or_else(|| MusicRepoError::NotFound("Playlist not found".to_string()))
    }

    /// Update a playlist (name/comment/songs).
    pub fn update_playlist(
        &self,
        playlist_id: i32,
        name: Option<&str>,
        comment: Option<&str>,
        public: Option<bool>,
        song_ids_to_add: &[i32],
        song_indices_to_remove: &[i32],
    ) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Update name/comment/public if provided
        if let Some(n) = name {
            diesel::update(playlists::table.filter(playlists::id.eq(playlist_id)))
                .set(playlists::name.eq(n))
                .execute(&mut conn)?;
        }

        if let Some(c) = comment {
            diesel::update(playlists::table.filter(playlists::id.eq(playlist_id)))
                .set(playlists::comment.eq(c))
                .execute(&mut conn)?;
        }

        if let Some(p) = public {
            diesel::update(playlists::table.filter(playlists::id.eq(playlist_id)))
                .set(playlists::public.eq(p))
                .execute(&mut conn)?;
        }

        // Remove songs by index (position)
        if !song_indices_to_remove.is_empty() {
            for index in song_indices_to_remove {
                diesel::delete(
                    playlist_songs::table
                        .filter(playlist_songs::playlist_id.eq(playlist_id))
                        .filter(playlist_songs::position.eq(index)),
                )
                .execute(&mut conn)?;
            }

            // Renumber positions
            Self::renumber_positions(&mut conn, playlist_id)?;
        }

        // Add new songs
        if !song_ids_to_add.is_empty() {
            // Get current max position
            let max_pos: Option<i32> = playlist_songs::table
                .filter(playlist_songs::playlist_id.eq(playlist_id))
                .select(diesel::dsl::max(playlist_songs::position))
                .first(&mut conn)?;

            let mut next_pos = max_pos.unwrap_or(-1) + 1;

            for song_id in song_ids_to_add {
                let new_song = NewPlaylistSong {
                    playlist_id,
                    song_id: *song_id,
                    position: next_pos,
                };

                diesel::insert_into(playlist_songs::table)
                    .values(&new_song)
                    .execute(&mut conn)?;

                next_pos += 1;
            }
        }

        // Update playlist stats
        Self::update_playlist_stats(&mut conn, playlist_id)?;

        Ok(())
    }

    /// Delete a playlist.
    pub fn delete_playlist(&self, playlist_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Delete playlist songs first (should cascade, but be explicit)
        diesel::delete(playlist_songs::table.filter(playlist_songs::playlist_id.eq(playlist_id)))
            .execute(&mut conn)?;

        // Delete playlist
        let deleted = diesel::delete(playlists::table.filter(playlists::id.eq(playlist_id)))
            .execute(&mut conn)?;

        Ok(deleted > 0)
    }

    /// Check if user owns a playlist.
    pub fn is_owner(&self, user_id: i32, playlist_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let owner_id: Option<i32> = playlists::table
            .filter(playlists::id.eq(playlist_id))
            .select(playlists::user_id)
            .first(&mut conn)
            .optional()?;

        Ok(owner_id == Some(user_id))
    }

    /// Helper to renumber positions after removal.
    fn renumber_positions(
        conn: &mut diesel::SqliteConnection,
        playlist_id: i32,
    ) -> Result<(), MusicRepoError> {
        // Get all playlist songs ordered by current position
        let song_ids: Vec<i32> = playlist_songs::table
            .filter(playlist_songs::playlist_id.eq(playlist_id))
            .order(playlist_songs::position.asc())
            .select(playlist_songs::id)
            .load(conn)?;

        // Update positions
        for (new_pos, id) in song_ids.iter().enumerate() {
            diesel::update(playlist_songs::table.filter(playlist_songs::id.eq(id)))
                .set(playlist_songs::position.eq(i32::try_from(new_pos).unwrap_or(0)))
                .execute(conn)?;
        }

        Ok(())
    }

    /// Helper to update playlist stats (`song_count`, duration).
    fn update_playlist_stats(
        conn: &mut diesel::SqliteConnection,
        playlist_id: i32,
    ) -> Result<(), MusicRepoError> {
        // Count songs and sum duration
        let results: Vec<SongRow> = playlist_songs::table
            .inner_join(songs::table.on(playlist_songs::song_id.eq(songs::id)))
            .filter(playlist_songs::playlist_id.eq(playlist_id))
            .select(SongRow::as_select())
            .load(conn)?;

        let song_count = i32::try_from(results.len()).unwrap_or(0);
        let total_duration: i32 = results.iter().map(|s| s.duration).sum();

        diesel::update(playlists::table.filter(playlists::id.eq(playlist_id)))
            .set((
                playlists::song_count.eq(song_count),
                playlists::duration.eq(total_duration),
                playlists::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .execute(conn)?;

        Ok(())
    }
}

// ============================================================================
// Play Queue Repository
// ============================================================================

/// Database row representation for play queue.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = play_queue)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PlayQueueRow {
    pub id: i32,
    pub user_id: i32,
    pub current_song_id: Option<i32>,
    pub position: Option<i64>,
    pub changed_at: NaiveDateTime,
    pub changed_by: Option<String>,
}

/// Database row representation for play queue songs.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = play_queue_songs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PlayQueueSongRow {
    pub id: i32,
    pub play_queue_id: i32,
    pub song_id: i32,
    pub position: i32,
}

/// Data for inserting a play queue.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = play_queue)]
pub struct NewPlayQueue {
    pub user_id: i32,
    pub current_song_id: Option<i32>,
    pub position: Option<i64>,
    pub changed_by: Option<String>,
}

/// Data for inserting a play queue song.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = play_queue_songs)]
pub struct NewPlayQueueSong {
    pub play_queue_id: i32,
    pub song_id: i32,
    pub position: i32,
}

/// Play queue with songs.
#[derive(Debug, Clone)]
pub struct PlayQueue {
    pub current_song: Option<Song>,
    pub position: Option<i64>,
    pub songs: Vec<Song>,
    pub changed_at: NaiveDateTime,
    pub changed_by: Option<String>,
    pub username: String,
}

/// Repository for play queue database operations.
#[derive(Clone, Debug)]
pub struct PlayQueueRepository {
    pool: DbPool,
}

impl PlayQueueRepository {
    /// Create a new play queue repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get the play queue for a user.
    pub fn get_play_queue(
        &self,
        user_id: i32,
        username: &str,
    ) -> Result<Option<PlayQueue>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Get the play queue
        let queue: Option<PlayQueueRow> = play_queue::table
            .filter(play_queue::user_id.eq(user_id))
            .select(PlayQueueRow::as_select())
            .first(&mut conn)
            .optional()?;

        let Some(queue) = queue else { return Ok(None) };

        // Get the current song
        let current_song = if let Some(song_id) = queue.current_song_id {
            songs::table
                .filter(songs::id.eq(song_id))
                .select(SongRow::as_select())
                .first(&mut conn)
                .optional()?
                .map(Song::from)
        } else {
            None
        };

        // Get all songs in the queue
        let song_rows: Vec<SongRow> = play_queue_songs::table
            .inner_join(songs::table.on(play_queue_songs::song_id.eq(songs::id)))
            .filter(play_queue_songs::play_queue_id.eq(queue.id))
            .order(play_queue_songs::position.asc())
            .select(SongRow::as_select())
            .load(&mut conn)?;

        let queue_songs: Vec<Song> = song_rows.into_iter().map(Song::from).collect();

        Ok(Some(PlayQueue {
            current_song,
            position: queue.position,
            songs: queue_songs,
            changed_at: queue.changed_at,
            changed_by: queue.changed_by,
            username: username.to_string(),
        }))
    }

    /// Save the play queue for a user.
    pub fn save_play_queue(
        &self,
        user_id: i32,
        song_ids: &[i32],
        current_song_id: Option<i32>,
        position: Option<i64>,
        changed_by: Option<&str>,
    ) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Get or create the play queue
        let queue_id: i32 = {
            let existing: Option<i32> = play_queue::table
                .filter(play_queue::user_id.eq(user_id))
                .select(play_queue::id)
                .first(&mut conn)
                .optional()?;

            if let Some(id) = existing {
                // Update existing queue
                diesel::update(play_queue::table.filter(play_queue::id.eq(id)))
                    .set((
                        play_queue::current_song_id.eq(current_song_id),
                        play_queue::position.eq(position),
                        play_queue::changed_at.eq(chrono::Utc::now().naive_utc()),
                        play_queue::changed_by.eq(changed_by),
                    ))
                    .execute(&mut conn)?;
                id
            } else {
                // Insert new queue
                let new_queue = NewPlayQueue {
                    user_id,
                    current_song_id,
                    position,
                    changed_by: changed_by.map(std::string::ToString::to_string),
                };

                diesel::insert_into(play_queue::table)
                    .values(&new_queue)
                    .execute(&mut conn)?;

                play_queue::table
                    .filter(play_queue::user_id.eq(user_id))
                    .select(play_queue::id)
                    .first(&mut conn)?
            }
        };

        // Clear existing songs
        diesel::delete(
            play_queue_songs::table.filter(play_queue_songs::play_queue_id.eq(queue_id)),
        )
        .execute(&mut conn)?;

        // Add new songs
        for (pos, song_id) in song_ids.iter().enumerate() {
            let new_song = NewPlayQueueSong {
                play_queue_id: queue_id,
                song_id: *song_id,
                position: i32::try_from(pos).unwrap_or(0),
            };

            diesel::insert_into(play_queue_songs::table)
                .values(&new_song)
                .execute(&mut conn)?;
        }

        Ok(())
    }
}
