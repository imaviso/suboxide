//! Subsonic API error codes and types.
//!
//! Error codes are defined by the Subsonic API specification.
//! See: <http://www.subsonic.org/pages/api.jsp>
//! `OpenSubsonic` extensions add additional error codes.

use axum::response::{IntoResponse, Response};
use thiserror::Error;

use super::response::{Format, error_response};

/// Subsonic API error codes.
/// These are defined by the Subsonic API specification.
/// `OpenSubsonic` extensions add codes 42-44.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ErrorCode {
    /// A generic error.
    Generic = 0,
    /// Required parameter is missing.
    MissingParameter = 10,
    /// Incompatible Subsonic REST protocol version. Client must upgrade.
    ClientTooOld = 20,
    /// Incompatible Subsonic REST protocol version. Server must upgrade.
    ServerTooOld = 30,
    /// Wrong username or password.
    WrongCredentials = 40,
    /// Token authentication not supported for LDAP users.
    TokenAuthNotSupported = 41,
    /// [`OpenSubsonic`] Provided authentication mechanism not supported.
    AuthMechanismNotSupported = 42,
    /// [`OpenSubsonic`] Multiple conflicting authentication mechanisms provided.
    ConflictingAuthMechanisms = 43,
    /// [`OpenSubsonic`] API key not valid.
    InvalidApiKey = 44,
    /// User is not authorized for the given operation.
    NotAuthorized = 50,
    /// The trial period for the Subsonic server is over.
    TrialExpired = 60,
    /// The requested data was not found.
    NotFound = 70,
}

/// API errors that can be returned to clients.
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("A generic error occurred: {0}")]
    Generic(String),

    #[error("Required parameter is missing: {0}")]
    MissingParameter(String),

    #[error("Client protocol version too old, please upgrade")]
    ClientTooOld,

    #[error("Server protocol version too old")]
    ServerTooOld,

    #[error("Wrong username or password")]
    WrongCredentials,

    #[error("Token authentication not supported for this user")]
    TokenAuthNotSupported,

    #[error("Provided authentication mechanism not supported")]
    AuthMechanismNotSupported,

    #[error("Multiple conflicting authentication mechanisms provided")]
    ConflictingAuthMechanisms,

    #[error("API key not valid")]
    InvalidApiKey,

    #[error("User is not authorized for the given operation")]
    NotAuthorized,

    #[error("Trial period is over")]
    TrialExpired,

    #[error("Requested data was not found: {0}")]
    NotFound(String),
}

impl ApiError {
    /// Get the Subsonic error code for this error.
    #[must_use] 
    pub const fn code(&self) -> u32 {
        match self {
            Self::Generic(_) => ErrorCode::Generic as u32,
            Self::MissingParameter(_) => ErrorCode::MissingParameter as u32,
            Self::ClientTooOld => ErrorCode::ClientTooOld as u32,
            Self::ServerTooOld => ErrorCode::ServerTooOld as u32,
            Self::WrongCredentials => ErrorCode::WrongCredentials as u32,
            Self::TokenAuthNotSupported => ErrorCode::TokenAuthNotSupported as u32,
            Self::AuthMechanismNotSupported => ErrorCode::AuthMechanismNotSupported as u32,
            Self::ConflictingAuthMechanisms => ErrorCode::ConflictingAuthMechanisms as u32,
            Self::InvalidApiKey => ErrorCode::InvalidApiKey as u32,
            Self::NotAuthorized => ErrorCode::NotAuthorized as u32,
            Self::TrialExpired => ErrorCode::TrialExpired as u32,
            Self::NotFound(_) => ErrorCode::NotFound as u32,
        }
    }

    /// Get the error message.
    #[must_use] 
    pub fn message(&self) -> String {
        self.to_string()
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        // Default to XML format when we don't have format context
        error_response(Format::Xml, &self).into_response()
    }
}
