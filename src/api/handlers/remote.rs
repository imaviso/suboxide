//! Remote control handlers (`OpenSubsonic` extension).

use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicAuth;
use crate::api::error::ApiError;
use crate::api::handlers::repo_error_response;
use crate::api::response::{SubsonicResponse, error_response};
use crate::db::{RemoteCommand, RemoteSession, RemoteState};
use crate::models::music::{
    RemoteCommandResponse, RemoteCommandsResponse, RemoteSessionResponse, RemoteStateResponse,
    format_subsonic_datetime,
};

const DEFAULT_REMOTE_SESSION_TTL_SECONDS: i64 = 60 * 5;
const MIN_REMOTE_SESSION_TTL_SECONDS: i64 = 60;
const MAX_REMOTE_SESSION_TTL_SECONDS: i64 = 60 * 60;

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CreateRemoteSessionParams {
    #[serde(rename = "deviceId")]
    pub device_id: Option<String>,
    #[serde(rename = "deviceName")]
    pub device_name: Option<String>,
    #[serde(rename = "ttlSeconds")]
    pub ttl_seconds: Option<i64>,
}

/// GET/POST /rest/createRemoteSession[.view]
///
/// Creates a new remote-control session and returns a pairing code.
pub async fn create_remote_session(
    axum::extract::Query(params): axum::extract::Query<CreateRemoteSessionParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(device_id) = resolve_device_id(params.device_id.as_deref(), &auth) else {
        return error_response(auth.format, &ApiError::MissingParameter("deviceId".into()))
            .into_response();
    };

    let ttl_seconds = params
        .ttl_seconds
        .unwrap_or(DEFAULT_REMOTE_SESSION_TTL_SECONDS)
        .clamp(
            MIN_REMOTE_SESSION_TTL_SECONDS,
            MAX_REMOTE_SESSION_TTL_SECONDS,
        );

    match auth.remote().create_remote_session(
        auth.user.id,
        device_id,
        params.device_name.as_deref(),
        ttl_seconds,
    ) {
        Ok(session) => SubsonicResponse::remote_session(auth.format, map_session(&session, true))
            .into_response(),
        Err(error) => repo_error_response(auth.format, error),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct JoinRemoteSessionParams {
    pub code: Option<String>,
    #[serde(rename = "deviceId")]
    pub device_id: Option<String>,
    #[serde(rename = "deviceName")]
    pub device_name: Option<String>,
}

/// GET/POST /rest/joinRemoteSession[.view]
///
/// Joins an existing remote-control session by pairing code.
pub async fn join_remote_session(
    axum::extract::Query(params): axum::extract::Query<JoinRemoteSessionParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(code) = params.code.as_deref().filter(|value| !value.is_empty()) else {
        return error_response(auth.format, &ApiError::MissingParameter("code".into()))
            .into_response();
    };

    let Some(device_id) = resolve_device_id(params.device_id.as_deref(), &auth) else {
        return error_response(auth.format, &ApiError::MissingParameter("deviceId".into()))
            .into_response();
    };

    match auth.remote().join_remote_session(
        auth.user.id,
        code,
        device_id,
        params.device_name.as_deref(),
    ) {
        Ok(Some(session)) => {
            SubsonicResponse::remote_session(auth.format, map_session(&session, false))
                .into_response()
        }
        Ok(None) => error_response(auth.format, &ApiError::NotFound("Remote session".into()))
            .into_response(),
        Err(error) => repo_error_response(auth.format, error),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CloseRemoteSessionParams {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GetRemoteSessionParams {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
}

/// GET/POST /rest/getRemoteSession[.view]
///
/// Returns metadata for an active remote-control session.
pub async fn get_remote_session(
    axum::extract::Query(params): axum::extract::Query<GetRemoteSessionParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return error_response(auth.format, &ApiError::MissingParameter("sessionId".into()))
            .into_response();
    };

    match auth.remote().get_remote_session(auth.user.id, session_id) {
        Ok(Some(session)) => {
            SubsonicResponse::remote_session(auth.format, map_session(&session, true))
                .into_response()
        }
        Ok(None) => error_response(auth.format, &ApiError::NotFound("Remote session".into()))
            .into_response(),
        Err(error) => repo_error_response(auth.format, error),
    }
}

/// GET/POST /rest/closeRemoteSession[.view]
///
/// Closes an active remote-control session.
pub async fn close_remote_session(
    axum::extract::Query(params): axum::extract::Query<CloseRemoteSessionParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return error_response(auth.format, &ApiError::MissingParameter("sessionId".into()))
            .into_response();
    };

    match auth.remote().close_remote_session(auth.user.id, session_id) {
        Ok(true) => SubsonicResponse::empty(auth.format).into_response(),
        Ok(false) => error_response(auth.format, &ApiError::NotFound("Remote session".into()))
            .into_response(),
        Err(error) => repo_error_response(auth.format, error),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SendRemoteCommandParams {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    pub command: Option<String>,
    pub payload: Option<String>,
    #[serde(rename = "deviceId")]
    pub device_id: Option<String>,
}

/// GET/POST /rest/sendRemoteCommand[.view]
///
/// Queues a command for the paired host device.
pub async fn send_remote_command(
    axum::extract::Query(params): axum::extract::Query<SendRemoteCommandParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return error_response(auth.format, &ApiError::MissingParameter("sessionId".into()))
            .into_response();
    };
    let Some(command) = params.command.as_deref().filter(|value| !value.is_empty()) else {
        return error_response(auth.format, &ApiError::MissingParameter("command".into()))
            .into_response();
    };
    let Some(device_id) = resolve_device_id(params.device_id.as_deref(), &auth) else {
        return error_response(auth.format, &ApiError::MissingParameter("deviceId".into()))
            .into_response();
    };

    match auth.remote().send_remote_command(
        auth.user.id,
        session_id,
        device_id,
        command,
        params.payload.as_deref(),
    ) {
        Ok(_) => SubsonicResponse::empty(auth.format).into_response(),
        Err(error) => repo_error_response(auth.format, error),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GetRemoteCommandsParams {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(rename = "sinceId")]
    pub since_id: Option<i64>,
    pub limit: Option<i64>,
    #[serde(rename = "deviceId")]
    pub device_id: Option<String>,
}

/// GET/POST /rest/getRemoteCommands[.view]
///
/// Returns queued commands for the current device.
pub async fn get_remote_commands(
    axum::extract::Query(params): axum::extract::Query<GetRemoteCommandsParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return error_response(auth.format, &ApiError::MissingParameter("sessionId".into()))
            .into_response();
    };
    let Some(device_id) = resolve_device_id(params.device_id.as_deref(), &auth) else {
        return error_response(auth.format, &ApiError::MissingParameter("deviceId".into()))
            .into_response();
    };

    let since_id = params.since_id.unwrap_or(0).max(0);
    let limit = params.limit.unwrap_or(100).clamp(1, 500);

    match auth
        .remote()
        .get_remote_commands(auth.user.id, session_id, since_id, limit, device_id)
    {
        Ok(commands) => {
            let response = RemoteCommandsResponse {
                commands: commands.iter().map(map_command).collect(),
            };
            SubsonicResponse::remote_commands(auth.format, response).into_response()
        }
        Err(error) => repo_error_response(auth.format, error),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct UpdateRemoteStateParams {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
    #[serde(rename = "stateJson")]
    pub state_json: Option<String>,
    #[serde(rename = "deviceId")]
    pub device_id: Option<String>,
}

/// GET/POST /rest/updateRemoteState[.view]
///
/// Updates the latest playback state for a remote session.
pub async fn update_remote_state(
    axum::extract::Query(params): axum::extract::Query<UpdateRemoteStateParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return error_response(auth.format, &ApiError::MissingParameter("sessionId".into()))
            .into_response();
    };
    let Some(state_json) = params
        .state_json
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return error_response(auth.format, &ApiError::MissingParameter("stateJson".into()))
            .into_response();
    };
    let Some(device_id) = resolve_device_id(params.device_id.as_deref(), &auth) else {
        return error_response(auth.format, &ApiError::MissingParameter("deviceId".into()))
            .into_response();
    };

    match auth
        .remote()
        .update_remote_state(auth.user.id, session_id, device_id, state_json)
    {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(error) => repo_error_response(auth.format, error),
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct GetRemoteStateParams {
    #[serde(rename = "sessionId")]
    pub session_id: Option<String>,
}

/// GET/POST /rest/getRemoteState[.view]
///
/// Returns the latest playback state for a remote session.
pub async fn get_remote_state(
    axum::extract::Query(params): axum::extract::Query<GetRemoteStateParams>,
    auth: SubsonicAuth,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return error_response(auth.format, &ApiError::MissingParameter("sessionId".into()))
            .into_response();
    };

    match auth.remote().get_remote_state(auth.user.id, session_id) {
        Ok(Some(state)) => {
            let response = map_state(&state);
            SubsonicResponse::remote_state(auth.format, response).into_response()
        }
        Ok(None) => {
            error_response(auth.format, &ApiError::NotFound("Remote state".into())).into_response()
        }
        Err(error) => repo_error_response(auth.format, error),
    }
}

fn resolve_device_id<'a>(requested: Option<&'a str>, auth: &'a SubsonicAuth) -> Option<&'a str> {
    requested
        .filter(|value| !value.is_empty())
        .or_else(|| (!auth.params.c.is_empty()).then_some(auth.params.c.as_str()))
}

fn map_session(session: &RemoteSession, include_pairing_code: bool) -> RemoteSessionResponse {
    RemoteSessionResponse {
        id: session.session_id.clone(),
        pairing_code: include_pairing_code
            .then(|| session.pairing_code.clone())
            .filter(|code| !code.is_empty()),
        expires_at: format_subsonic_datetime(&session.expires_at),
        host_device_id: session.host_device_id.clone(),
        host_device_name: session.host_device_name.clone(),
        controller_device_id: session.controller_device_id.clone(),
        controller_device_name: session.controller_device_name.clone(),
        connected: session.controller_device_id.is_some(),
    }
}

fn map_command(command: &RemoteCommand) -> RemoteCommandResponse {
    RemoteCommandResponse {
        id: command.id,
        command: command.command.clone(),
        payload: command.payload.clone(),
        source_device_id: command.source_device_id.clone(),
        created: format_subsonic_datetime(&command.created_at),
    }
}

fn map_state(state: &RemoteState) -> RemoteStateResponse {
    RemoteStateResponse {
        state_json: state.state_json.clone(),
        updated_by_device_id: state.updated_by_device_id.clone(),
        updated_at: format_subsonic_datetime(&state.updated_at),
    }
}
