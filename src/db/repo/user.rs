//! User repository operations.

use diesel::prelude::*;

use crate::db::DbPool;
use crate::db::repo::error::{UserRepoError, UserRepoErrorKind};
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
    pub lastfm_session_key: Option<String>,
}

impl From<UserRow> for User {
    fn from(row: UserRow) -> Self {
        Self {
            id: row.id,
            username: row.username,
            password_hash: row.password_hash,
            subsonic_password: row.subsonic_password,
            api_key: row.api_key,
            lastfm_session_key: row.lastfm_session_key,
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

/// Builder for `NewUser`.
#[derive(Debug, Clone)]
#[expect(clippy::struct_excessive_bools)]
pub struct NewUserBuilder<'a> {
    username: &'a str,
    password_hash: &'a str,
    subsonic_password: Option<&'a str>,
    email: Option<&'a str>,
    admin_role: bool,
    settings_role: bool,
    stream_role: bool,
    jukebox_role: bool,
    download_role: bool,
    upload_role: bool,
    playlist_role: bool,
    cover_art_role: bool,
    comment_role: bool,
    podcast_role: bool,
    share_role: bool,
    video_conversion_role: bool,
    max_bit_rate: i32,
}

impl<'a> NewUserBuilder<'a> {
    /// Create a new builder with required fields.
    #[must_use]
    pub const fn new(username: &'a str, password_hash: &'a str) -> Self {
        Self {
            username,
            password_hash,
            subsonic_password: None,
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

    /// Set the subsonic password.
    #[must_use]
    pub const fn subsonic_password(mut self, password: &'a str) -> Self {
        self.subsonic_password = Some(password);
        self
    }

    /// Set the email.
    #[must_use]
    pub const fn email(mut self, email: &'a str) -> Self {
        self.email = Some(email);
        self
    }

    /// Set admin role.
    #[must_use]
    pub const fn admin_role(mut self, admin: bool) -> Self {
        self.admin_role = admin;
        self
    }

    /// Set settings role.
    #[must_use]
    pub const fn settings_role(mut self, settings: bool) -> Self {
        self.settings_role = settings;
        self
    }

    /// Set stream role.
    #[must_use]
    pub const fn stream_role(mut self, stream: bool) -> Self {
        self.stream_role = stream;
        self
    }

    /// Set jukebox role.
    #[must_use]
    pub const fn jukebox_role(mut self, jukebox: bool) -> Self {
        self.jukebox_role = jukebox;
        self
    }

    /// Set download role.
    #[must_use]
    pub const fn download_role(mut self, download: bool) -> Self {
        self.download_role = download;
        self
    }

    /// Set upload role.
    #[must_use]
    pub const fn upload_role(mut self, upload: bool) -> Self {
        self.upload_role = upload;
        self
    }

    /// Set playlist role.
    #[must_use]
    pub const fn playlist_role(mut self, playlist: bool) -> Self {
        self.playlist_role = playlist;
        self
    }

    /// Set cover art role.
    #[must_use]
    pub const fn cover_art_role(mut self, cover_art: bool) -> Self {
        self.cover_art_role = cover_art;
        self
    }

    /// Set comment role.
    #[must_use]
    pub const fn comment_role(mut self, comment: bool) -> Self {
        self.comment_role = comment;
        self
    }

    /// Set podcast role.
    #[must_use]
    pub const fn podcast_role(mut self, podcast: bool) -> Self {
        self.podcast_role = podcast;
        self
    }

    /// Set share role.
    #[must_use]
    pub const fn share_role(mut self, share: bool) -> Self {
        self.share_role = share;
        self
    }

    /// Set video conversion role.
    #[must_use]
    pub const fn video_conversion_role(mut self, video: bool) -> Self {
        self.video_conversion_role = video;
        self
    }

    /// Set max bit rate.
    #[must_use]
    pub const fn max_bit_rate(mut self, rate: i32) -> Self {
        self.max_bit_rate = rate;
        self
    }

    /// Build the `NewUser`.
    #[must_use]
    pub const fn build(self) -> NewUser<'a> {
        NewUser {
            username: self.username,
            password_hash: self.password_hash,
            subsonic_password: self.subsonic_password,
            email: self.email,
            admin_role: self.admin_role,
            settings_role: self.settings_role,
            stream_role: self.stream_role,
            jukebox_role: self.jukebox_role,
            download_role: self.download_role,
            upload_role: self.upload_role,
            playlist_role: self.playlist_role,
            cover_art_role: self.cover_art_role,
            comment_role: self.comment_role,
            podcast_role: self.podcast_role,
            share_role: self.share_role,
            video_conversion_role: self.video_conversion_role,
            max_bit_rate: self.max_bit_rate,
        }
    }
}

impl<'a> NewUser<'a> {
    /// Create a builder for `NewUser`.
    #[must_use]
    pub const fn builder(username: &'a str, password_hash: &'a str) -> NewUserBuilder<'a> {
        NewUserBuilder::new(username, password_hash)
    }

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
            return Err(UserRepoError::new(
                UserRepoErrorKind::UsernameExists,
                new_user.username.to_string(),
            ));
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

    /// Set or update a user's Last.fm session key.
    pub fn set_lastfm_session_key(
        &self,
        user_id: i32,
        session_key: Option<&str>,
    ) -> Result<bool, UserRepoError> {
        let mut conn = self.pool.get()?;

        let updated = diesel::update(users::table.filter(users::id.eq(user_id)))
            .set(users::lastfm_session_key.eq(session_key))
            .execute(&mut conn)?;

        Ok(updated > 0)
    }

    /// Get a user's Last.fm session key.
    pub fn get_lastfm_session_key(&self, user_id: i32) -> Result<Option<String>, UserRepoError> {
        let mut conn = self.pool.get()?;

        let result = users::table
            .filter(users::id.eq(user_id))
            .select(users::lastfm_session_key)
            .first::<Option<String>>(&mut conn)
            .optional()?;

        Ok(result.flatten())
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
            .ok_or_else(|| {
                UserRepoError::new(UserRepoErrorKind::NotFound, update.username.clone())
            })?;

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
                update
                    .lastfm_session_key
                    .as_ref()
                    .map(|sk| users::lastfm_session_key.eq(Some(sk.as_str()))),
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
    pub lastfm_session_key: Option<String>,
}

/// Builder for `UserUpdate`.
#[derive(Debug, Clone)]
pub struct UserUpdateBuilder {
    username: String,
    email: Option<String>,
    admin_role: Option<bool>,
    settings_role: Option<bool>,
    stream_role: Option<bool>,
    jukebox_role: Option<bool>,
    download_role: Option<bool>,
    upload_role: Option<bool>,
    playlist_role: Option<bool>,
    cover_art_role: Option<bool>,
    comment_role: Option<bool>,
    podcast_role: Option<bool>,
    share_role: Option<bool>,
    video_conversion_role: Option<bool>,
    max_bit_rate: Option<i32>,
    lastfm_session_key: Option<String>,
}

impl UserUpdateBuilder {
    /// Create a new builder for updating a user.
    pub fn new(username: impl Into<String>) -> Self {
        Self {
            username: username.into(),
            email: None,
            admin_role: None,
            settings_role: None,
            stream_role: None,
            jukebox_role: None,
            download_role: None,
            upload_role: None,
            playlist_role: None,
            cover_art_role: None,
            comment_role: None,
            podcast_role: None,
            share_role: None,
            video_conversion_role: None,
            max_bit_rate: None,
            lastfm_session_key: None,
        }
    }

    /// Set the Last.fm session key.
    #[must_use]
    pub fn lastfm_session_key(mut self, sk: impl Into<String>) -> Self {
        self.lastfm_session_key = Some(sk.into());
        self
    }

    #[must_use]
    pub fn email(mut self, email: impl Into<String>) -> Self {
        self.email = Some(email.into());
        self
    }

    #[must_use]
    pub const fn admin_role(mut self, val: bool) -> Self {
        self.admin_role = Some(val);
        self
    }

    #[must_use]
    pub const fn settings_role(mut self, val: bool) -> Self {
        self.settings_role = Some(val);
        self
    }

    #[must_use]
    pub const fn stream_role(mut self, val: bool) -> Self {
        self.stream_role = Some(val);
        self
    }

    #[must_use]
    pub const fn jukebox_role(mut self, val: bool) -> Self {
        self.jukebox_role = Some(val);
        self
    }

    #[must_use]
    pub const fn download_role(mut self, val: bool) -> Self {
        self.download_role = Some(val);
        self
    }

    #[must_use]
    pub const fn upload_role(mut self, val: bool) -> Self {
        self.upload_role = Some(val);
        self
    }

    #[must_use]
    pub const fn playlist_role(mut self, val: bool) -> Self {
        self.playlist_role = Some(val);
        self
    }

    #[must_use]
    pub const fn cover_art_role(mut self, val: bool) -> Self {
        self.cover_art_role = Some(val);
        self
    }

    #[must_use]
    pub const fn comment_role(mut self, val: bool) -> Self {
        self.comment_role = Some(val);
        self
    }

    #[must_use]
    pub const fn podcast_role(mut self, val: bool) -> Self {
        self.podcast_role = Some(val);
        self
    }

    #[must_use]
    pub const fn share_role(mut self, val: bool) -> Self {
        self.share_role = Some(val);
        self
    }

    #[must_use]
    pub const fn video_conversion_role(mut self, val: bool) -> Self {
        self.video_conversion_role = Some(val);
        self
    }

    #[must_use]
    pub const fn max_bit_rate(mut self, val: i32) -> Self {
        self.max_bit_rate = Some(val);
        self
    }

    #[must_use]
    pub fn build(self) -> UserUpdate {
        UserUpdate {
            username: self.username,
            email: self.email,
            admin_role: self.admin_role,
            settings_role: self.settings_role,
            stream_role: self.stream_role,
            jukebox_role: self.jukebox_role,
            download_role: self.download_role,
            upload_role: self.upload_role,
            playlist_role: self.playlist_role,
            cover_art_role: self.cover_art_role,
            comment_role: self.comment_role,
            podcast_role: self.podcast_role,
            share_role: self.share_role,
            video_conversion_role: self.video_conversion_role,
            max_bit_rate: self.max_bit_rate,
            lastfm_session_key: self.lastfm_session_key,
        }
    }
}

impl UserUpdate {
    /// Create a builder for `UserUpdate`.
    pub fn builder(username: impl Into<String>) -> UserUpdateBuilder {
        UserUpdateBuilder::new(username)
    }
}
