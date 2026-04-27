//! User management API handlers (getUser, getUsers, deleteUser, changePassword, createUser, updateUser)
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::{AuthParams, SubsonicAuth};
use crate::api::error::ApiError;
use crate::api::response::{SubsonicResponse, error_response};
use crate::models::user::{UserResponse, UserRoles, UsersResponse};

/// Query parameters for getUser.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GetUserParams {
    /// The name of the user to retrieve.
    pub username: Option<String>,
}

/// GET/POST /rest/getUser[.view]
///
/// Get details about a given user, including which authorization roles and folder access it has.
/// Can be used to get information about the currently logged in user.
pub async fn get_user(
    axum::extract::Query(params): axum::extract::Query<GetUserParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let username = match &params.username {
        Some(u) => u.as_str(),
        None => {
            return error_response(auth.format, &ApiError::MissingParameter("username".into()));
        }
    };

    // Non-admins can only query their own user
    if !auth.user.is_admin() && username != auth.user.username {
        return error_response(auth.format, &ApiError::NotAuthorized);
    }

    match auth.state.get_user(username) {
        Some(user) => {
            let response = UserResponse::from(&user);
            SubsonicResponse::user(auth.format, response)
        }
        None => error_response(auth.format, &ApiError::NotFound("User not found".into())),
    }
}

/// GET/POST /rest/getUsers[.view]
///
/// Get details about all users, including which authorization roles and folder access they have.
/// Only users with admin role are allowed to call this method.
pub async fn get_users(auth: SubsonicAuth) -> impl IntoResponse {
    if !auth.user.is_admin() {
        return error_response(auth.format, &ApiError::NotAuthorized);
    }

    let users = auth.state.get_all_users();
    let user_responses: Vec<UserResponse> = users.iter().map(UserResponse::from).collect();

    let response = UsersResponse {
        users: user_responses,
    };

    SubsonicResponse::users(auth.format, response)
}

/// Query parameters for deleteUser.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DeleteUserParams {
    /// The name of the user to delete.
    pub username: Option<String>,
}

/// GET/POST /rest/deleteUser[.view]
///
/// Deletes an existing user.
/// Only users with admin role are allowed to call this method.
pub async fn delete_user(
    axum::extract::Query(params): axum::extract::Query<DeleteUserParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    if !auth.user.is_admin() {
        return error_response(auth.format, &ApiError::NotAuthorized);
    }

    let username = match &params.username {
        Some(u) => u.as_str(),
        None => {
            return error_response(auth.format, &ApiError::MissingParameter("username".into()));
        }
    };

    // Prevent deleting yourself
    if username == auth.user.username {
        return error_response(
            auth.format,
            &ApiError::Generic("Cannot delete your own user".into()),
        );
    }

    match auth.state.delete_user(username) {
        Ok(true) => SubsonicResponse::empty(auth.format),
        Ok(false) => error_response(auth.format, &ApiError::NotFound("User not found".into())),
        Err(e) => error_response(auth.format, &ApiError::Generic(e)),
    }
}

/// Query parameters for changePassword.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ChangePasswordParams {
    /// The name of the user to change password for.
    pub username: Option<String>,
    /// The new password of the user (can be hex-encoded with "enc:" prefix).
    pub password: Option<String>,
}

/// GET/POST /rest/changePassword[.view]
///
/// Changes the password of an existing user.
/// Non-admin users can only change their own password.
/// Admins can change anyone's password.
pub async fn change_password(
    axum::extract::Query(params): axum::extract::Query<ChangePasswordParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let username = match &params.username {
        Some(u) => u.as_str(),
        None => {
            return error_response(auth.format, &ApiError::MissingParameter("username".into()));
        }
    };

    let password = match &params.password {
        Some(p) => p.as_str(),
        None => {
            return error_response(auth.format, &ApiError::MissingParameter("password".into()));
        }
    };

    // Non-admins can only change their own password
    if !auth.user.is_admin() && username != auth.user.username {
        return error_response(auth.format, &ApiError::NotAuthorized);
    }

    // Decode password if hex-encoded
    let Some(decoded_password) = AuthParams::decode_password(password) else {
        return error_response(auth.format, &ApiError::WrongCredentials);
    };

    match auth.state.change_password(username, &decoded_password) {
        Ok(()) => SubsonicResponse::empty(auth.format),
        Err(e) => error_response(auth.format, &ApiError::Generic(e)),
    }
}

/// Query parameters for createUser.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CreateUserParams {
    /// The name of the new user.
    pub username: Option<String>,
    /// The password of the new user (can be hex-encoded with "enc:" prefix).
    pub password: Option<String>,
    /// The email address of the new user.
    pub email: Option<String>,
    /// Whether the user is administrator.
    pub admin_role: Option<bool>,
    /// Whether the user is allowed to change personal settings and password.
    pub settings_role: Option<bool>,
    /// Whether the user is allowed to play files.
    pub stream_role: Option<bool>,
    /// Whether the user is allowed to play files in jukebox mode.
    pub jukebox_role: Option<bool>,
    /// Whether the user is allowed to download files.
    pub download_role: Option<bool>,
    /// Whether the user is allowed to upload files.
    pub upload_role: Option<bool>,
    /// Whether the user is allowed to create and delete playlists.
    pub playlist_role: Option<bool>,
    /// Whether the user is allowed to change cover art and tags.
    pub cover_art_role: Option<bool>,
    /// Whether the user is allowed to create and edit comments and ratings.
    pub comment_role: Option<bool>,
    /// Whether the user is allowed to administrate Podcasts.
    pub podcast_role: Option<bool>,
    /// Whether the user is allowed to share files with anyone.
    pub share_role: Option<bool>,
    /// Whether the user is allowed to start video conversions.
    pub video_conversion_role: Option<bool>,
}

/// GET/POST /rest/createUser[.view]
///
/// Creates a new user on the server.
/// Only users with admin role are allowed to call this method.
pub async fn create_user(
    axum::extract::Query(params): axum::extract::Query<CreateUserParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    if !auth.user.is_admin() {
        return error_response(auth.format, &ApiError::NotAuthorized);
    }

    let username = match &params.username {
        Some(u) => u.as_str(),
        None => {
            return error_response(auth.format, &ApiError::MissingParameter("username".into()));
        }
    };

    let password = match &params.password {
        Some(p) => p.as_str(),
        None => {
            return error_response(auth.format, &ApiError::MissingParameter("password".into()));
        }
    };

    let email = match &params.email {
        Some(e) => e.as_str(),
        None => {
            return error_response(auth.format, &ApiError::MissingParameter("email".into()));
        }
    };

    // Decode password if hex-encoded
    let Some(decoded_password) = AuthParams::decode_password(password) else {
        return error_response(auth.format, &ApiError::WrongCredentials);
    };

    // Apply default values per the Subsonic API spec
    let roles = UserRoles {
        admin_role: params.admin_role.unwrap_or(false),
        settings_role: params.settings_role.unwrap_or(true),
        stream_role: params.stream_role.unwrap_or(true),
        jukebox_role: params.jukebox_role.unwrap_or(false),
        download_role: params.download_role.unwrap_or(false),
        upload_role: params.upload_role.unwrap_or(false),
        playlist_role: params.playlist_role.unwrap_or(false),
        cover_art_role: params.cover_art_role.unwrap_or(false),
        comment_role: params.comment_role.unwrap_or(false),
        podcast_role: params.podcast_role.unwrap_or(false),
        share_role: params.share_role.unwrap_or(false),
        video_conversion_role: params.video_conversion_role.unwrap_or(false),
    };

    match auth
        .state
        .create_user(username, &decoded_password, email, roles)
    {
        Ok(_) => SubsonicResponse::empty(auth.format),
        Err(e) => error_response(auth.format, &ApiError::Generic(e)),
    }
}

/// Query parameters for updateUser.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct UpdateUserParams {
    /// The name of the user to update.
    pub username: Option<String>,
    /// The password of the user (can be hex-encoded with "enc:" prefix).
    pub password: Option<String>,
    /// The email address of the user.
    pub email: Option<String>,
    /// Whether the user is administrator.
    pub admin_role: Option<bool>,
    /// Whether the user is allowed to change personal settings and password.
    pub settings_role: Option<bool>,
    /// Whether the user is allowed to play files.
    pub stream_role: Option<bool>,
    /// Whether the user is allowed to play files in jukebox mode.
    pub jukebox_role: Option<bool>,
    /// Whether the user is allowed to download files.
    pub download_role: Option<bool>,
    /// Whether the user is allowed to upload files.
    pub upload_role: Option<bool>,
    /// Whether the user is allowed to create and delete playlists.
    pub playlist_role: Option<bool>,
    /// Whether the user is allowed to change cover art and tags.
    pub cover_art_role: Option<bool>,
    /// Whether the user is allowed to create and edit comments and ratings.
    pub comment_role: Option<bool>,
    /// Whether the user is allowed to administrate Podcasts.
    pub podcast_role: Option<bool>,
    /// Whether the user is allowed to share files with anyone.
    pub share_role: Option<bool>,
    /// Whether the user is allowed to start video conversions.
    pub video_conversion_role: Option<bool>,
    /// The maximum bit rate (in Kbps) for the user.
    pub max_bit_rate: Option<i32>,
}

/// GET/POST /rest/updateUser[.view]
///
/// Modifies an existing user on the server.
/// Only users with admin role are allowed to call this method.
pub async fn update_user(
    axum::extract::Query(params): axum::extract::Query<UpdateUserParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    if !auth.user.is_admin() {
        return error_response(auth.format, &ApiError::NotAuthorized);
    }

    let username = match &params.username {
        Some(u) => u.as_str(),
        None => {
            return error_response(auth.format, &ApiError::MissingParameter("username".into()));
        }
    };

    // Decode password if provided and hex-encoded
    let decoded_password = match params.password.as_deref().map(AuthParams::decode_password) {
        Some(Some(password)) => Some(password),
        Some(None) => return error_response(auth.format, &ApiError::WrongCredentials),
        None => None,
    };

    match auth.state.update_user(
        username,
        decoded_password.as_deref(),
        params.email.as_deref(),
        params.admin_role,
        params.settings_role,
        params.stream_role,
        params.jukebox_role,
        params.download_role,
        params.upload_role,
        params.playlist_role,
        params.cover_art_role,
        params.comment_role,
        params.podcast_role,
        params.share_role,
        params.video_conversion_role,
        params.max_bit_rate,
    ) {
        Ok(()) => SubsonicResponse::empty(auth.format),
        Err(e) => error_response(auth.format, &ApiError::Generic(e)),
    }
}
