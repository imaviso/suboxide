//! Cryptographic utilities.

pub mod password;

#[doc(inline)]
pub use password::{PasswordError, hash_password, verify_password};
