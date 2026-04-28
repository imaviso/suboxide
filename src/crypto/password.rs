//! Password hashing using Argon2.
//!
//! This module provides secure password hashing and verification using the Argon2id
//! algorithm, which is the recommended choice for password hashing.

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};

/// Kind of password error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordErrorKind {
    /// Failed to hash password.
    Hash,
    /// Failed to verify password.
    Verify,
    /// Invalid password hash format.
    InvalidHash,
}

impl std::fmt::Display for PasswordErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hash => write!(f, "failed to hash password"),
            Self::Verify => write!(f, "failed to verify password"),
            Self::InvalidHash => write!(f, "invalid password hash format"),
        }
    }
}

/// Errors that can occur during password operations.
#[derive(Debug)]
pub struct PasswordError {
    kind: PasswordErrorKind,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl PasswordError {
    /// Create a new password error.
    #[must_use]
    pub fn new(kind: PasswordErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Returns the error kind.
    #[must_use]
    pub const fn kind(&self) -> PasswordErrorKind {
        self.kind
    }

    /// Returns the error message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for PasswordError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.kind, self.message)
    }
}

impl std::error::Error for PasswordError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}

/// Hash a password using Argon2id.
///
/// Returns a PHC-formatted string that includes the algorithm parameters and salt.
///
/// # Example
///
/// ```
/// use suboxide::crypto::password::hash_password;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let hash = hash_password("my_secure_password")?;
/// assert!(hash.starts_with("$argon2id$"));
/// # Ok(())
/// # }
/// ```
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();

    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|e| PasswordError::new(PasswordErrorKind::Hash, e.to_string()))
}

/// Verify a password against a stored hash.
///
/// # Arguments
///
/// * `password` - The plaintext password to verify
/// * `hash` - The PHC-formatted hash string to verify against
///
/// # Returns
///
/// `true` if the password matches, `false` otherwise.
///
/// # Example
///
/// ```
/// use suboxide::crypto::password::{hash_password, verify_password};
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let hash = hash_password("my_password")?;
/// assert!(verify_password("my_password", &hash)?);
/// assert!(!verify_password("wrong_password", &hash)?);
/// # Ok(())
/// # }
/// ```
pub fn verify_password(password: &str, hash: &str) -> Result<bool, PasswordError> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|error| PasswordError::new(PasswordErrorKind::InvalidHash, error.to_string()))?;

    let argon2 = Argon2::default();

    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(PasswordError::new(PasswordErrorKind::Verify, e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password() -> Result<(), PasswordError> {
        let hash = hash_password("test_password")?;
        assert!(hash.starts_with("$argon2id$"));
        assert!(hash.len() > 50); // Argon2 hashes are fairly long
        Ok(())
    }

    #[test]
    fn test_verify_correct_password() -> Result<(), PasswordError> {
        let hash = hash_password("correct_password")?;
        assert!(verify_password("correct_password", &hash)?);
        Ok(())
    }

    #[test]
    fn test_verify_wrong_password() -> Result<(), PasswordError> {
        let hash = hash_password("correct_password")?;
        assert!(!verify_password("wrong_password", &hash)?);
        Ok(())
    }

    #[test]
    fn test_different_passwords_different_hashes() -> Result<(), PasswordError> {
        let hash1 = hash_password("password1")?;
        let hash2 = hash_password("password1")?;
        // Same password should produce different hashes (different salts)
        assert_ne!(hash1, hash2);
        Ok(())
    }

    #[test]
    fn test_invalid_hash_format() {
        let result = verify_password("password", "not_a_valid_hash");
        assert_eq!(result.unwrap_err().kind(), PasswordErrorKind::InvalidHash);
    }

    #[test]
    fn invalid_hash_error_includes_parse_context() {
        let result = verify_password("password", "not_a_valid_hash");

        let err = result.expect_err("invalid hash should return error");
        assert_eq!(err.kind(), PasswordErrorKind::InvalidHash);
        assert!(!err.message().is_empty());
        assert_ne!(err.message(), "not_a_valid_hash");
    }
}
