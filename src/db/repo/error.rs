//! Repository errors.

use thiserror::Error;

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
