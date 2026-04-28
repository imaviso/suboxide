//! Authentication middleware and extractors for Subsonic API.
//!
//! Subsonic supports multiple authentication methods:
//! 1. Legacy: Plain password sent via `p` parameter (deprecated)
//! 2. Token: MD5(password + salt) sent via `t` and `s` parameters
//! 3. API Key (OpenSubsonic): API key sent via `apiKey` parameter
//!
//! For username/password auth, all API requests must include:
//! - `u`: Username
//! - `v`: Client API version
//! - `c`: Client name/identifier
//! - Either `p` (password) or `t` + `s` (token + salt)
//!
//! For API key auth:
//! - `apiKey`: The API key (must NOT include `u` parameter)
//! - `v`: Client API version
//! - `c`: Client name/identifier
//!
//! Parameters can be passed via:
//! - Query string (GET requests)
//! - Form body (POST requests with application/x-www-form-urlencoded)
//! - Or a combination of both (query params take precedence)

use std::sync::Arc;

use axum::{
    Form,
    body::Body,
    extract::{FromRef, FromRequest, Query, Request},
    http::Method,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use super::error::ApiError;
use super::response::{Format, error_response};
use super::services::{MusicService, RemoteControlService, UserService};
use crate::db::{DbPool, UserRepoError};
use crate::models::User;
use crate::scanner::ScanStateHandle;

/// User lookup required by authentication.
pub trait AuthState: Send + Sync + 'static {
    /// Find a user by username.
    fn find_user(&self, username: &str) -> Result<Option<User>, UserRepoError>;
    /// Find a user by API key.
    fn find_user_by_api_key(&self, api_key: &str) -> Result<Option<User>, UserRepoError>;
}

impl AuthState for UserService {
    fn find_user(&self, username: &str) -> Result<Option<User>, UserRepoError> {
        Self::find_user(self, username)
    }

    fn find_user_by_api_key(&self, api_key: &str) -> Result<Option<User>, UserRepoError> {
        Self::find_user_by_api_key(self, api_key)
    }
}

/// Shared authentication state handle.
#[derive(Clone)]
pub struct AuthStateHandle(Arc<UserService>);

impl AuthStateHandle {
    /// Create a shared state handle.
    #[must_use]
    pub const fn new(state: Arc<UserService>) -> Self {
        Self(state)
    }
}

impl std::fmt::Debug for AuthStateHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AuthStateHandle")
            .field(&"<dyn AuthState>")
            .finish()
    }
}

impl std::ops::Deref for AuthStateHandle {
    type Target = UserService;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Common query parameters for all Subsonic API requests.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AuthParams {
    /// Username
    #[serde(alias = "u")]
    pub u: String,
    /// Password (legacy, deprecated) - either hex-encoded with "enc:" prefix or plain
    #[serde(alias = "p")]
    pub p: Option<String>,
    /// Authentication token = MD5(password + salt)
    #[serde(alias = "t")]
    pub t: Option<String>,
    /// Salt used to generate the token
    #[serde(alias = "s")]
    pub s: Option<String>,
    /// API key (`OpenSubsonic` extension)
    #[serde(alias = "apiKey")]
    pub api_key: Option<String>,
    /// Client API version
    #[serde(alias = "v")]
    pub v: String,
    /// Client identifier
    #[serde(alias = "c")]
    pub c: String,
    /// Response format (xml, json, jsonp)
    #[serde(alias = "f")]
    pub f: Option<String>,
}

impl AuthParams {
    /// Get the response format.
    #[must_use]
    pub fn format(&self) -> Format {
        Format::from_param(self.f.as_deref())
    }

    /// Decode password if it's hex-encoded (prefixed with "enc:").
    #[must_use]
    pub fn decode_password(password: &str) -> Option<String> {
        password.strip_prefix("enc:").map_or_else(
            || Some(password.to_string()),
            |hex_encoded| {
                hex::decode(hex_encoded)
                    .ok()
                    .and_then(|bytes| String::from_utf8(bytes).ok())
            },
        )
    }

    /// Merge with another `AuthParams`, taking non-empty values from self.
    /// This is used to combine query params (higher priority) with form params.
    #[must_use]
    pub fn merge_with(mut self, other: Self) -> Self {
        if self.u.is_empty() {
            self.u = other.u;
        }
        if self.p.is_none() {
            self.p = other.p;
        }
        if self.t.is_none() {
            self.t = other.t;
        }
        if self.s.is_none() {
            self.s = other.s;
        }
        if self.api_key.is_none() {
            self.api_key = other.api_key;
        }
        if self.v.is_empty() {
            self.v = other.v;
        }
        if self.c.is_empty() {
            self.c = other.c;
        }
        if self.f.is_none() {
            self.f = other.f;
        }
        self
    }

    /// Check if API key auth is being used.
    #[must_use]
    pub const fn uses_api_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// Check if username/password auth is being used.
    #[must_use]
    pub const fn uses_user_auth(&self) -> bool {
        !self.u.is_empty() || self.p.is_some() || self.t.is_some()
    }
}

/// Authenticated user extractor that also includes the response format.
///
/// Supports GET and POST authentication parameters.
/// Endpoint parameters are still read from query strings.
///
/// Use this in your handlers to require authentication:
///
/// ```ignore
/// async fn handler(auth: SubsonicAuth) -> impl IntoResponse {
///     // auth.user is guaranteed to be authenticated
///     // auth.format contains the requested response format
///     ok_empty(auth.format)
/// }
/// ```
#[derive(Clone)]
pub struct SubsonicAuth {
    /// The authenticated user.
    pub user: User,
    /// The requested response format.
    pub format: Format,
    /// Common Subsonic authentication parameters.
    pub params: AuthParams,
    music: MusicService,
    users: UserService,
    remote: RemoteControlService,
    scan_state: ScanStateHandle,
    pool: DbPool,
}

impl SubsonicAuth {
    /// Return the music service.
    #[must_use]
    pub const fn music(&self) -> &MusicService {
        &self.music
    }

    /// Return the user service.
    #[must_use]
    pub const fn users(&self) -> &UserService {
        &self.users
    }

    /// Return the remote control service.
    #[must_use]
    pub const fn remote(&self) -> &RemoteControlService {
        &self.remote
    }

    /// Return the scan state handle.
    #[must_use]
    pub const fn scan_state(&self) -> &ScanStateHandle {
        &self.scan_state
    }

    /// Return the database pool.
    #[must_use]
    pub const fn pool(&self) -> &DbPool {
        &self.pool
    }
}

impl std::fmt::Debug for SubsonicAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubsonicAuth")
            .field("user", &self.user)
            .field("format", &self.format)
            .field("params", &self.params)
            .finish_non_exhaustive()
    }
}

/// Error wrapper that includes format information for proper error responses.
#[derive(Debug)]
pub struct AuthError {
    pub error: ApiError,
    pub format: Format,
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        error_response(self.format, &self.error).into_response()
    }
}

impl<S> FromRequest<S> for SubsonicAuth
where
    S: Send + Sync,
    AuthStateHandle: FromRef<S>,
    MusicService: FromRef<S>,
    UserService: FromRef<S>,
    RemoteControlService: FromRef<S>,
    ScanStateHandle: FromRef<S>,
    DbPool: FromRef<S>,
{
    type Rejection = AuthError;

    #[expect(
        clippy::too_many_lines,
        reason = "Extractor validates multiple auth flows and transports in one place"
    )]
    async fn from_request(req: Request<Body>, state: &S) -> Result<Self, Self::Rejection> {
        let is_post = req.method() == Method::POST;

        // Extract query parameters first (they exist in both GET and POST)
        let (parts, body) = req.into_parts();
        let query_params = Query::<AuthParams>::try_from_uri(&parts.uri)
            .map(|q| q.0)
            .unwrap_or_default();

        // For POST requests, also extract form body parameters
        let mut params = if is_post {
            // Reconstruct the request to extract form data
            let req = Request::from_parts(parts.clone(), body);
            match Form::<AuthParams>::from_request(req, state).await {
                Ok(Form(form_params)) => query_params.merge_with(form_params),
                Err(e) => {
                    tracing::warn!(error = %e, "form auth parameter parsing failed");
                    return Err(AuthError {
                        error: ApiError::MissingParameter("valid form body".into()),
                        format: query_params.format(),
                    });
                }
            }
        } else {
            query_params
        };

        // Support for clients passing credentials in HTTP headers (e.g. SolidSonic)
        // Checks for X-Subsonic-Username, X-Subsonic-Token, and X-Subsonic-Salt
        #[expect(
            clippy::collapsible_if,
            reason = "Nested checks keep header parsing flow explicit"
        )]
        if params.u.is_empty() {
            if let Some(Ok(u)) = parts.headers.get("X-Subsonic-Username").map(|h| h.to_str()) {
                params.u = u.to_string();

                if let Some(Ok(t)) = parts.headers.get("X-Subsonic-Token").map(|h| h.to_str()) {
                    params.t = Some(t.to_string());
                }

                if let Some(Ok(s)) = parts.headers.get("X-Subsonic-Salt").map(|h| h.to_str()) {
                    params.s = Some(s.to_string());
                }
            }
        }

        let format = params.format();

        // Validate common required parameters (for all auth methods)
        if params.v.is_empty() {
            return Err(AuthError {
                error: ApiError::MissingParameter("v (version)".into()),
                format,
            });
        }
        if params.c.is_empty() {
            return Err(AuthError {
                error: ApiError::MissingParameter("c (client)".into()),
                format,
            });
        }

        // Get auth state
        let auth_state = AuthStateHandle::from_ref(state);

        // Check for conflicting auth mechanisms
        if params.uses_api_key() && params.uses_user_auth() {
            return Err(AuthError {
                error: ApiError::ConflictingAuthMechanisms,
                format,
            });
        }

        // Authenticate based on the method used
        if let Some(api_key) = &params.api_key {
            // API Key authentication (OpenSubsonic extension)
            // When using API key, username must NOT be provided
            if !params.u.is_empty() {
                return Err(AuthError {
                    error: ApiError::ConflictingAuthMechanisms,
                    format,
                });
            }

            let user = auth_state
                .find_user_by_api_key(api_key)
                .map_err(|error| AuthError {
                    error: ApiError::Generic(error.to_string()),
                    format,
                })?
                .ok_or(AuthError {
                    error: ApiError::InvalidApiKey,
                    format,
                })?;

            Ok(Self {
                user,
                format,
                params,
                music: MusicService::from_ref(state),
                users: UserService::from_ref(state),
                remote: RemoteControlService::from_ref(state),
                scan_state: ScanStateHandle::from_ref(state),
                pool: DbPool::from_ref(state),
            })
        } else {
            // Username/password or token authentication
            if params.u.is_empty() {
                return Err(AuthError {
                    error: ApiError::MissingParameter("u (username) or apiKey".into()),
                    format,
                });
            }

            // Find user by username
            let user = auth_state
                .find_user(&params.u)
                .map_err(|error| AuthError {
                    error: ApiError::Generic(error.to_string()),
                    format,
                })?
                .ok_or(AuthError {
                    error: ApiError::WrongCredentials,
                    format,
                })?;

            // Authenticate using token or password
            let authenticated = if let (Some(token), Some(salt)) = (&params.t, &params.s) {
                // Token authentication (preferred by many clients)
                user.verify_token(token, salt)
            } else if let Some(password) = &params.p {
                // Legacy password authentication - use Argon2
                AuthParams::decode_password(password)
                    .is_some_and(|decoded| user.verify_password(&decoded).unwrap_or(false))
            } else {
                return Err(AuthError {
                    error: ApiError::MissingParameter(
                        "authentication: 'apiKey', 'p' (password), or 't' and 's' (token and salt)"
                            .into(),
                    ),
                    format,
                });
            };

            if authenticated {
                Ok(Self {
                    user,
                    format,
                    params,
                    music: MusicService::from_ref(state),
                    users: UserService::from_ref(state),
                    remote: RemoteControlService::from_ref(state),
                    scan_state: ScanStateHandle::from_ref(state),
                    pool: DbPool::from_ref(state),
                })
            } else {
                Err(AuthError {
                    error: ApiError::WrongCredentials,
                    format,
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_encoded_password() {
        // "password" in hex is "70617373776f7264"
        let encoded = "enc:70617373776f7264";
        let decoded = AuthParams::decode_password(encoded);
        assert_eq!(decoded.as_deref(), Some("password"));

        // Plain password should be returned as-is
        let plain = "password";
        assert_eq!(
            AuthParams::decode_password(plain).as_deref(),
            Some("password")
        );
    }

    #[test]
    fn test_params_merge() {
        let query = AuthParams {
            u: "user".into(),
            v: "1.16.1".into(),
            c: "test".into(),
            p: Some("pass".into()),
            ..Default::default()
        };
        let form = AuthParams {
            u: "other".into(),
            v: "1.15.0".into(),
            c: "other_client".into(),
            f: Some("json".into()),
            ..Default::default()
        };

        let merged = query.merge_with(form);

        // Query params should take precedence
        assert_eq!(merged.u, "user");
        assert_eq!(merged.v, "1.16.1");
        assert_eq!(merged.c, "test");
        assert_eq!(merged.p, Some("pass".into()));
        // Form params fill in missing values
        assert_eq!(merged.f, Some("json".into()));
    }

    #[test]
    fn params_merge_preserves_query_auth_fields_and_fills_missing_form_fields() {
        let query = AuthParams {
            u: "query-user".into(),
            t: Some("query-token".into()),
            v: "1.16.1".into(),
            c: "query-client".into(),
            ..Default::default()
        };
        let form = AuthParams {
            u: "form-user".into(),
            p: Some("form-password".into()),
            t: Some("form-token".into()),
            s: Some("form-salt".into()),
            api_key: Some("form-key".into()),
            v: "1.15.0".into(),
            c: "form-client".into(),
            f: Some("json".into()),
        };

        let merged = query.merge_with(form);

        assert_eq!(merged.u, "query-user");
        assert_eq!(merged.t.as_deref(), Some("query-token"));
        assert_eq!(merged.v, "1.16.1");
        assert_eq!(merged.c, "query-client");
        assert_eq!(merged.p.as_deref(), Some("form-password"));
        assert_eq!(merged.s.as_deref(), Some("form-salt"));
        assert_eq!(merged.api_key.as_deref(), Some("form-key"));
        assert_eq!(merged.f.as_deref(), Some("json"));
    }

    #[test]
    fn invalid_hex_password_returns_none() {
        assert_eq!(AuthParams::decode_password("enc:not-hex"), None);
        assert_eq!(AuthParams::decode_password("enc:ff"), None);
    }

    #[test]
    fn test_api_key_detection() {
        let with_api_key = AuthParams {
            api_key: Some("secret".into()),
            v: "1.16.1".into(),
            c: "test".into(),
            ..Default::default()
        };
        assert!(with_api_key.uses_api_key());
        assert!(!with_api_key.uses_user_auth());

        let with_user = AuthParams {
            u: "user".into(),
            p: Some("pass".into()),
            v: "1.16.1".into(),
            c: "test".into(),
            ..Default::default()
        };
        assert!(!with_user.uses_api_key());
        assert!(with_user.uses_user_auth());
    }
}
