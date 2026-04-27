//! Repository errors.

use std::fmt;
use thiserror::Error;

/// Error kind for user repository operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserRepoErrorKind {
    /// Database query failed.
    Database,
    /// Connection pool error.
    Pool,
    /// The requested user was not found.
    NotFound,
    /// The username already exists.
    UsernameExists,
}

impl fmt::Display for UserRepoErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database => write!(f, "database error"),
            Self::Pool => write!(f, "connection pool error"),
            Self::NotFound => write!(f, "user not found"),
            Self::UsernameExists => write!(f, "username already exists"),
        }
    }
}

/// Canonical error type for user repository operations.
#[derive(Debug, Error)]
#[error("{kind}: {message}")]
pub struct UserRepoError {
    kind: UserRepoErrorKind,
    message: String,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl UserRepoError {
    /// Create a new user repository error.
    pub fn new(kind: UserRepoErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Create a new user repository error with a source.
    pub fn with_source(
        kind: UserRepoErrorKind,
        message: impl Into<String>,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// Returns the error kind.
    #[must_use]
    pub const fn kind(&self) -> UserRepoErrorKind {
        self.kind
    }

    /// Returns the error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<diesel::result::Error> for UserRepoError {
    fn from(err: diesel::result::Error) -> Self {
        Self::with_source(UserRepoErrorKind::Database, err.to_string(), err)
    }
}

impl From<diesel::r2d2::PoolError> for UserRepoError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        Self::with_source(UserRepoErrorKind::Pool, err.to_string(), err)
    }
}

/// Error kind for music repository operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MusicRepoErrorKind {
    /// Database query failed.
    Database,
    /// Connection pool error.
    Pool,
    /// The requested item was not found.
    NotFound,
    /// The item already exists.
    AlreadyExists,
}

impl fmt::Display for MusicRepoErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database => write!(f, "database error"),
            Self::Pool => write!(f, "connection pool error"),
            Self::NotFound => write!(f, "not found"),
            Self::AlreadyExists => write!(f, "already exists"),
        }
    }
}

/// Canonical error type for music library repository operations.
#[derive(Debug, Error)]
#[error("{kind}: {message}")]
pub struct MusicRepoError {
    kind: MusicRepoErrorKind,
    message: String,
    #[source]
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl MusicRepoError {
    /// Create a new music repository error.
    pub fn new(kind: MusicRepoErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Create a new music repository error with a source.
    pub fn with_source(
        kind: MusicRepoErrorKind,
        message: impl Into<String>,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self {
            kind,
            message: message.into(),
            source: Some(source.into()),
        }
    }

    /// Returns the error kind.
    #[must_use]
    pub const fn kind(&self) -> MusicRepoErrorKind {
        self.kind
    }

    /// Returns the error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<diesel::result::Error> for MusicRepoError {
    fn from(err: diesel::result::Error) -> Self {
        Self::with_source(MusicRepoErrorKind::Database, err.to_string(), err)
    }
}

impl From<diesel::r2d2::PoolError> for MusicRepoError {
    fn from(err: diesel::r2d2::PoolError) -> Self {
        Self::with_source(MusicRepoErrorKind::Pool, err.to_string(), err)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io;

    use super::{MusicRepoError, MusicRepoErrorKind, UserRepoError, UserRepoErrorKind};

    #[test]
    fn user_repo_error_preserves_kind_message_and_source() {
        let error = UserRepoError::with_source(
            UserRepoErrorKind::UsernameExists,
            "duplicate username",
            io::Error::new(io::ErrorKind::AlreadyExists, "user exists"),
        );

        assert_eq!(error.kind(), UserRepoErrorKind::UsernameExists);
        assert_eq!(error.message(), "duplicate username");
        assert_eq!(
            error.to_string(),
            "username already exists: duplicate username"
        );
        assert!(error.source().is_some());
    }

    #[test]
    fn music_repo_error_without_source_is_deterministic() {
        let error = MusicRepoError::new(MusicRepoErrorKind::NotFound, "album missing");

        assert_eq!(error.kind(), MusicRepoErrorKind::NotFound);
        assert_eq!(error.message(), "album missing");
        assert_eq!(error.to_string(), "not found: album missing");
        assert!(error.source().is_none());
    }
}
