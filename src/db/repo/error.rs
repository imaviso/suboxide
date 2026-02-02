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
    pub kind: UserRepoErrorKind,
    pub message: String,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
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
    pub kind: MusicRepoErrorKind,
    pub message: String,
    #[source]
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
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
