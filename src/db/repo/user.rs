//! User repository operations.

use diesel::prelude::*;

use crate::db::DbPool;
use crate::db::repo::error::UserRepoError;
use crate::db::schema::users;
use crate::models::User;
use crate::models::user::UserRoles;

/// Database row representation for users.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = users)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[expect(clippy::struct_excessive_bools)]
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
#[expect(clippy::struct_excessive_bools)]
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
