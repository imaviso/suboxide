//! Shared handler response helpers.

use axum::response::{IntoResponse, Response};

use crate::api::auth::SubsonicContext;
use crate::api::error::ApiError;
use crate::api::response::error_response;

/// Build a missing-parameter response for the current request format.
pub(in crate::api::handlers) fn missing_param(auth: &SubsonicContext, name: &str) -> Response {
    api_error(auth, &ApiError::MissingParameter(name.into()))
}

/// Build a not-found response for the current request format.
pub(in crate::api::handlers) fn not_found(auth: &SubsonicContext, resource: &str) -> Response {
    api_error(auth, &ApiError::NotFound(resource.into()))
}

/// Build a not-authorized response for the current request format.
pub(in crate::api::handlers) fn unauthorized(auth: &SubsonicContext) -> Response {
    api_error(auth, &ApiError::NotAuthorized)
}

/// Build a generic service error response for the current request format.
pub(in crate::api::handlers) fn service_error(
    auth: &SubsonicContext,
    error: impl std::fmt::Display,
) -> Response {
    api_error(auth, &ApiError::Generic(error.to_string()))
}

/// Build a formatted API error response.
pub(in crate::api::handlers) fn api_error(auth: &SubsonicContext, error: &ApiError) -> Response {
    error_response(auth.format, error).into_response()
}
