//! System-related API handlers (ping, getLicense, getOpenSubsonicExtensions, tokenInfo, etc.)

use axum::response::IntoResponse;

use crate::api::auth::SubsonicContext;
use crate::api::response::{SubsonicResponse, supported_extensions};
use crate::models::music::TokenInfoResponse;

/// GET/POST /rest/ping[.view]
///
/// Used to test connectivity with the server.
/// Returns an empty successful response.
pub async fn ping(auth: SubsonicContext) -> impl IntoResponse {
    SubsonicResponse::empty(auth.format)
}

/// GET/POST /rest/getLicense[.view]
///
/// Get details about the software license.
/// Since this is an open-source implementation, we always return valid.
pub async fn get_license(auth: SubsonicContext) -> impl IntoResponse {
    SubsonicResponse::license(auth.format)
}

/// GET/POST /rest/getOpenSubsonicExtensions[.view]
///
/// List the `OpenSubsonic` extensions supported by this server.
/// This endpoint is part of the `OpenSubsonic` specification.
pub async fn get_open_subsonic_extensions(auth: SubsonicContext) -> impl IntoResponse {
    SubsonicResponse::open_subsonic_extensions(auth.format, supported_extensions())
}

/// GET/POST /rest/getBookmarks[.view]
///
/// Returns all bookmarks for this user.
/// A bookmark is a position within a certain media file.
/// Currently returns an empty list (bookmarks not yet implemented).
pub async fn get_bookmarks(auth: SubsonicContext) -> impl IntoResponse {
    SubsonicResponse::bookmarks(auth.format)
}

/// GET/POST /rest/tokenInfo[.view]
///
/// Returns information about the API key used for authentication.
/// This is an `OpenSubsonic` extension.
///
/// Returns the username associated with the API key.
pub async fn token_info(auth: SubsonicContext) -> impl IntoResponse {
    let response = TokenInfoResponse {
        username: auth.user.username.clone(),
    };
    SubsonicResponse::token_info(auth.format, response)
}
