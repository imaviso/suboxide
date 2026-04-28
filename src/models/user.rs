//! User model and related types.

use serde::Serialize;

use crate::crypto::password::{PasswordError, verify_password};

/// User roles/permissions.
#[derive(Debug, Clone, Default, Serialize)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Subsonic roles are represented as independent boolean flags in the protocol"
)]
pub struct UserRoles {
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
}

/// A user in the system (domain model).
#[derive(Debug, Clone)]
pub struct User {
    pub id: i32,
    pub username: String,
    /// Argon2 hashed password.
    pub password_hash: String,
    /// Plaintext password stored for Subsonic token auth (MD5-based).
    /// This is needed because Subsonic's token auth requires: MD5(password + salt)
    /// where password is the original plaintext password.
    /// In a more secure setup, you might store a separate "`subsonic_token`" field.
    pub subsonic_password: Option<String>,
    /// API key for `OpenSubsonic` apiKeyAuthentication extension.
    pub api_key: Option<String>,
    /// Last.fm session key for scrobbling.
    pub lastfm_session_key: Option<String>,
    pub email: Option<String>,
    pub roles: UserRoles,
    /// Max bitrate for streaming (0 = unlimited).
    pub max_bit_rate: i32,
}

impl User {
    /// Check if user is an admin.
    #[must_use]
    pub const fn is_admin(&self) -> bool {
        self.roles.admin_role
    }

    /// Check if user has Last.fm configured.
    #[must_use]
    pub const fn has_lastfm(&self) -> bool {
        self.lastfm_session_key.is_some()
    }

    /// Verify password using Argon2.
    ///
    /// # Errors
    /// Returns `Err` if the stored hash is malformed (data corruption),
    /// not if the password is wrong.
    pub fn verify_password(&self, password: &str) -> Result<bool, PasswordError> {
        verify_password(password, &self.password_hash)
    }

    /// Verify password using Subsonic token authentication (MD5).
    /// Token = MD5(password + salt)
    ///
    /// Note: This requires storing the plaintext password or a dedicated token.
    /// For better security, consider using the password directly with Argon2
    /// and requiring clients to use the password auth method instead of tokens.
    #[must_use]
    pub fn verify_token(&self, token: &str, salt: &str) -> bool {
        self.subsonic_password.as_ref().is_some_and(|password| {
            use md5::{Digest, Md5};
            let mut hasher = Md5::new();
            hasher.update(password.as_bytes());
            hasher.update(salt.as_bytes());
            let expected_token = hex::encode(hasher.finalize());
            expected_token.eq_ignore_ascii_case(token)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::crypto::password::PasswordErrorKind;

    use super::User;

    #[test]
    fn verify_password_returns_err_for_malformed_hash() {
        let user = User {
            id: 1,
            username: "test".into(),
            password_hash: "not_a_valid_argon2_hash".into(),
            subsonic_password: None,
            api_key: None,
            lastfm_session_key: None,
            email: None,
            roles: Default::default(),
            max_bit_rate: 0,
        };

        let result = user.verify_password("any_password");
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), PasswordErrorKind::InvalidHash);
    }
}

/// Subsonic API user response format.
#[derive(Debug, Serialize, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Subsonic response schema exposes each role as a dedicated boolean attribute"
)]
pub struct UserResponse {
    #[serde(rename = "@username")]
    pub username: String,
    #[serde(rename = "@email", skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    #[serde(rename = "@scrobblingEnabled")]
    pub scrobbling_enabled: bool,
    #[serde(rename = "@adminRole")]
    pub admin_role: bool,
    #[serde(rename = "@settingsRole")]
    pub settings_role: bool,
    #[serde(rename = "@streamRole")]
    pub stream_role: bool,
    #[serde(rename = "@jukeboxRole")]
    pub jukebox_role: bool,
    #[serde(rename = "@downloadRole")]
    pub download_role: bool,
    #[serde(rename = "@uploadRole")]
    pub upload_role: bool,
    #[serde(rename = "@playlistRole")]
    pub playlist_role: bool,
    #[serde(rename = "@coverArtRole")]
    pub cover_art_role: bool,
    #[serde(rename = "@commentRole")]
    pub comment_role: bool,
    #[serde(rename = "@podcastRole")]
    pub podcast_role: bool,
    #[serde(rename = "@shareRole")]
    pub share_role: bool,
    #[serde(rename = "@videoConversionRole")]
    pub video_conversion_role: bool,
    #[serde(rename = "@maxBitRate")]
    pub max_bit_rate: i32,
}

impl From<&User> for UserResponse {
    fn from(user: &User) -> Self {
        Self {
            username: user.username.clone(),
            email: user.email.clone(),
            scrobbling_enabled: user.has_lastfm(),
            admin_role: user.roles.admin_role,
            settings_role: user.roles.settings_role,
            stream_role: user.roles.stream_role,
            jukebox_role: user.roles.jukebox_role,
            download_role: user.roles.download_role,
            upload_role: user.roles.upload_role,
            playlist_role: user.roles.playlist_role,
            cover_art_role: user.roles.cover_art_role,
            comment_role: user.roles.comment_role,
            podcast_role: user.roles.podcast_role,
            share_role: user.roles.share_role,
            video_conversion_role: user.roles.video_conversion_role,
            max_bit_rate: user.max_bit_rate,
        }
    }
}

/// Subsonic API users response format for getUsers.
#[derive(Debug, Serialize, Clone)]
pub struct UsersResponse {
    #[serde(rename = "user", skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<UserResponse>,
}
