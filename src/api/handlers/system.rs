//! System-related API handlers (ping, getLicense, getOpenSubsonicExtensions, tokenInfo, etc.)

#![allow(clippy::unused_async)]

use axum::response::IntoResponse;

use crate::api::auth::SubsonicAuth;
use crate::api::response::{
    ok_bookmarks, ok_empty, ok_license, ok_open_subsonic_extensions, ok_token_info,
};
use crate::models::music::TokenInfoResponse;

/// GET/POST /rest/ping[.view]
///
/// Used to test connectivity with the server.
/// Returns an empty successful response.
pub async fn ping(auth: SubsonicAuth) -> impl IntoResponse {
    ok_empty(auth.format)
}

/// GET/POST /rest/getLicense[.view]
///
/// Get details about the software license.
/// Since this is an open-source implementation, we always return valid.
pub async fn get_license(auth: SubsonicAuth) -> impl IntoResponse {
    ok_license(auth.format)
}

/// GET/POST /rest/getOpenSubsonicExtensions[.view]
///
/// List the `OpenSubsonic` extensions supported by this server.
/// This endpoint is part of the `OpenSubsonic` specification.
pub async fn get_open_subsonic_extensions(auth: SubsonicAuth) -> impl IntoResponse {
    ok_open_subsonic_extensions(auth.format)
}

/// GET/POST /rest/getBookmarks[.view]
///
/// Returns all bookmarks for this user.
/// A bookmark is a position within a certain media file.
/// Currently returns an empty list (bookmarks not yet implemented).
pub async fn get_bookmarks(auth: SubsonicAuth) -> impl IntoResponse {
    ok_bookmarks(auth.format)
}

/// GET/POST /rest/tokenInfo[.view]
///
/// Returns information about the API key used for authentication.
/// This is an `OpenSubsonic` extension.
///
/// Returns the username associated with the API key.
pub async fn token_info(auth: SubsonicAuth) -> impl IntoResponse {
    let response = TokenInfoResponse {
        username: auth.user.username.clone(),
    };
    ok_token_info(auth.format, response)
}
