//! Password hashing using Argon2.
//!
//! This module provides secure password hashing and verification using the Argon2id
//! algorithm, which is the recommended choice for password hashing.

use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use thiserror::Error;

/// Errors that can occur during password operations.
#[derive(Debug, Error)]
pub enum PasswordError {
    #[error("Failed to hash password: {0}")]
    HashError(String),

    #[error("Failed to verify password: {0}")]
    VerifyError(String),

    #[error("Invalid password hash format: {0}")]
    InvalidHash(String),
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
        .map_err(|e| PasswordError::HashError(e.to_string()))
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
    let parsed_hash =
        PasswordHash::new(hash).map_err(|error| PasswordError::InvalidHash(error.to_string()))?;

    let argon2 = Argon2::default();

    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(PasswordError::VerifyError(e.to_string())),
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
        assert!(matches!(result, Err(PasswordError::InvalidHash(_))));
    }

    #[test]
    fn invalid_hash_error_includes_parse_context() {
        let result = verify_password("password", "not_a_valid_hash");

        let Err(PasswordError::InvalidHash(message)) = result else {
            panic!("invalid hash should return InvalidHash");
        };

        assert!(!message.is_empty());
        assert_ne!(message, "not_a_valid_hash");
    }
}
