//! Internet radio station handlers.
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicContext;
use crate::api::handlers::util;
use crate::api::response::SubsonicResponse;
use crate::models::music::{InternetRadioStationResponse, InternetRadioStationsResponse};

/// GET/POST /rest/getInternetRadioStations[.view]
///
/// Returns all internet radio stations.
pub async fn get_internet_radio_stations(auth: SubsonicContext) -> impl IntoResponse {
    let stations = match auth.music().get_internet_radio_stations() {
        Ok(stations) => stations,
        Err(error) => return util::repo_error(&auth, error),
    };

    let stations: Vec<InternetRadioStationResponse> = stations
        .iter()
        .map(|station| InternetRadioStationResponse {
            id: station.id.to_string(),
            name: station.name.clone(),
            stream_url: station.stream_url.clone(),
            home_page_url: station.home_page_url.clone(),
        })
        .collect();

    SubsonicResponse::internet_radio_stations(
        auth.format,
        InternetRadioStationsResponse { stations },
    )
    .into_response()
}

/// Query parameters for createInternetRadioStation/updateInternetRadioStation.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InternetRadioStationParams {
    /// Station ID (required for update).
    pub id: Option<String>,
    /// The stream URL for the station.
    #[serde(rename = "streamUrl")]
    pub stream_url: Option<String>,
    /// The user-friendly name of the station.
    pub name: Option<String>,
    /// The home page URL for the station.
    #[serde(rename = "homepageUrl")]
    pub home_page_url: Option<String>,
}

/// GET/POST /rest/createInternetRadioStation[.view]
///
/// Adds a new internet radio station. Admin only.
pub async fn create_internet_radio_station(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<
        InternetRadioStationParams,
    >,
    auth: SubsonicContext,
) -> impl IntoResponse {
    if !auth.user.is_admin() {
        return util::unauthorized(&auth);
    }

    let Some(stream_url) = params.stream_url.as_deref() else {
        return util::missing_param(&auth, "streamUrl");
    };
    let Some(name) = params.name.as_deref() else {
        return util::missing_param(&auth, "name");
    };

    match auth.music().create_internet_radio_station(
        name,
        stream_url,
        params.home_page_url.as_deref(),
    ) {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(error) => util::repo_error(&auth, error),
    }
}

/// GET/POST /rest/updateInternetRadioStation[.view]
///
/// Updates an existing internet radio station. Admin only.
pub async fn update_internet_radio_station(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<
        InternetRadioStationParams,
    >,
    auth: SubsonicContext,
) -> impl IntoResponse {
    if !auth.user.is_admin() {
        return util::unauthorized(&auth);
    }

    let Some(id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return util::missing_param(&auth, "id");
    };
    let Some(stream_url) = params.stream_url.as_deref() else {
        return util::missing_param(&auth, "streamUrl");
    };
    let Some(name) = params.name.as_deref() else {
        return util::missing_param(&auth, "name");
    };

    match auth.music().update_internet_radio_station(
        id,
        name,
        stream_url,
        params.home_page_url.as_deref(),
    ) {
        Ok(true) => SubsonicResponse::empty(auth.format).into_response(),
        Ok(false) => util::not_found(&auth, "Internet radio station"),
        Err(error) => util::repo_error(&auth, error),
    }
}

/// Query parameters for deleteInternetRadioStation.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DeleteInternetRadioStationParams {
    /// Station ID.
    pub id: Option<String>,
}

/// GET/POST /rest/deleteInternetRadioStation[.view]
///
/// Deletes an existing internet radio station. Admin only.
pub async fn delete_internet_radio_station(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<
        DeleteInternetRadioStationParams,
    >,
    auth: SubsonicContext,
) -> impl IntoResponse {
    if !auth.user.is_admin() {
        return util::unauthorized(&auth);
    }

    let Some(id) = params.id.as_ref().and_then(|id| id.parse::<i32>().ok()) else {
        return util::missing_param(&auth, "id");
    };

    match auth.music().delete_internet_radio_station(id) {
        Ok(true) => SubsonicResponse::empty(auth.format).into_response(),
        Ok(false) => util::not_found(&auth, "Internet radio station"),
        Err(error) => util::repo_error(&auth, error),
    }
}
