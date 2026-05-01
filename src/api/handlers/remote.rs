//! Remote control handlers (`OpenSubsonic` extension).

use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicContext;
use crate::api::handlers::util;

use crate::api::response::SubsonicResponse;
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<
        CreateRemoteSessionParams,
    >,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(device_id) = resolve_device_id(params.device_id.as_deref(), &auth) else {
        return util::missing_param(&auth, "deviceId");
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
        Err(error) => util::service_error(&auth, error),
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<
        JoinRemoteSessionParams,
    >,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(code) = params.code.as_deref().filter(|value| !value.is_empty()) else {
        return util::missing_param(&auth, "code");
    };

    let Some(device_id) = resolve_device_id(params.device_id.as_deref(), &auth) else {
        return util::missing_param(&auth, "deviceId");
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
        Ok(None) => util::not_found(&auth, "Remote session"),
        Err(error) => util::service_error(&auth, error),
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<
        GetRemoteSessionParams,
    >,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return util::missing_param(&auth, "sessionId");
    };

    match auth.remote().get_remote_session(auth.user.id, session_id) {
        Ok(Some(session)) => {
            SubsonicResponse::remote_session(auth.format, map_session(&session, true))
                .into_response()
        }
        Ok(None) => util::not_found(&auth, "Remote session"),
        Err(error) => util::service_error(&auth, error),
    }
}

/// GET/POST /rest/closeRemoteSession[.view]
///
/// Closes an active remote-control session.
pub async fn close_remote_session(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<
        CloseRemoteSessionParams,
    >,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return util::missing_param(&auth, "sessionId");
    };

    match auth.remote().close_remote_session(auth.user.id, session_id) {
        Ok(true) => SubsonicResponse::empty(auth.format).into_response(),
        Ok(false) => util::not_found(&auth, "Remote session"),
        Err(error) => util::service_error(&auth, error),
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<
        SendRemoteCommandParams,
    >,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return util::missing_param(&auth, "sessionId");
    };
    let Some(command) = params.command.as_deref().filter(|value| !value.is_empty()) else {
        return util::missing_param(&auth, "command");
    };
    let Some(device_id) = resolve_device_id(params.device_id.as_deref(), &auth) else {
        return util::missing_param(&auth, "deviceId");
    };

    match auth.remote().send_remote_command(
        auth.user.id,
        session_id,
        device_id,
        command,
        params.payload.as_deref(),
    ) {
        Ok(_) => SubsonicResponse::empty(auth.format).into_response(),
        Err(error) => util::service_error(&auth, error),
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<
        GetRemoteCommandsParams,
    >,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return util::missing_param(&auth, "sessionId");
    };
    let Some(device_id) = resolve_device_id(params.device_id.as_deref(), &auth) else {
        return util::missing_param(&auth, "deviceId");
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
        Err(error) => util::service_error(&auth, error),
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<
        UpdateRemoteStateParams,
    >,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return util::missing_param(&auth, "sessionId");
    };
    let Some(state_json) = params
        .state_json
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return util::missing_param(&auth, "stateJson");
    };
    let Some(device_id) = resolve_device_id(params.device_id.as_deref(), &auth) else {
        return util::missing_param(&auth, "deviceId");
    };

    match auth
        .remote()
        .update_remote_state(auth.user.id, session_id, device_id, state_json)
    {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(error) => util::service_error(&auth, error),
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
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<GetRemoteStateParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(session_id) = params
        .session_id
        .as_deref()
        .filter(|value| !value.is_empty())
    else {
        return util::missing_param(&auth, "sessionId");
    };

    match auth.remote().get_remote_state(auth.user.id, session_id) {
        Ok(Some(state)) => {
            let response = map_state(&state);
            SubsonicResponse::remote_state(auth.format, response).into_response()
        }
        Ok(None) => util::not_found(&auth, "Remote state"),
        Err(error) => util::service_error(&auth, error),
    }
}

fn resolve_device_id<'a>(requested: Option<&'a str>, auth: &'a SubsonicContext) -> Option<&'a str> {
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

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::{map_command, map_session, map_state};
    use crate::db::{RemoteCommand, RemoteSession, RemoteState};

    fn ts() -> chrono::NaiveDateTime {
        NaiveDate::from_ymd_opt(2024, 1, 2)
            .expect("valid date")
            .and_hms_milli_opt(3, 4, 5, 678)
            .expect("valid time")
    }

    fn remote_session(controller_device_id: Option<&str>) -> RemoteSession {
        RemoteSession {
            session_id: "session-1".to_string(),
            pairing_code: "PAIR12".to_string(),
            owner_user_id: 7,
            host_device_id: "host-device".to_string(),
            host_device_name: Some("Host".to_string()),
            controller_user_id: controller_device_id.map(|_| 8),
            controller_device_id: controller_device_id.map(str::to_string),
            controller_device_name: controller_device_id.map(|_| "Controller".to_string()),
            expires_at: ts(),
            created_at: ts(),
            updated_at: ts(),
            closed_at: None,
        }
    }

    #[test]
    fn map_session_includes_pairing_code_only_when_requested_and_marks_connected() {
        let disconnected = map_session(&remote_session(None), true);
        assert_eq!(disconnected.pairing_code.as_deref(), Some("PAIR12"));
        assert!(!disconnected.connected);

        let connected = map_session(&remote_session(Some("controller-device")), false);
        assert_eq!(connected.pairing_code, None);
        assert!(connected.connected);
        assert_eq!(
            connected.controller_device_id.as_deref(),
            Some("controller-device")
        );
        assert_eq!(connected.expires_at, "2024-01-02T03:04:05.678Z");
    }

    #[test]
    fn map_command_preserves_payload_source_and_timestamp() {
        let response = map_command(&RemoteCommand {
            id: 42,
            command: "play".to_string(),
            payload: Some(r#"{"id":"song-1"}"#.to_string()),
            source_device_id: "controller-device".to_string(),
            created_at: ts(),
        });

        assert_eq!(response.id, 42);
        assert_eq!(response.command, "play");
        assert_eq!(response.payload.as_deref(), Some(r#"{"id":"song-1"}"#));
        assert_eq!(response.source_device_id, "controller-device");
        assert_eq!(response.created, "2024-01-02T03:04:05.678Z");
    }

    #[test]
    fn map_state_preserves_json_device_and_timestamp() {
        let response = map_state(&RemoteState {
            state_json: r#"{"playing":true}"#.to_string(),
            updated_by_device_id: "host-device".to_string(),
            updated_at: ts(),
        });

        assert_eq!(response.state_json, r#"{"playing":true}"#);
        assert_eq!(response.updated_by_device_id, "host-device");
        assert_eq!(response.updated_at, "2024-01-02T03:04:05.678Z");
    }
}
