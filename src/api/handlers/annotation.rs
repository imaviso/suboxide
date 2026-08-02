//! Annotation-related API handlers (star, unstar, getStarred2, scrobble, getNowPlaying, setRating, etc.)
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::api::auth::SubsonicContext;
use crate::api::handlers::util;

use crate::api::response::SubsonicResponse;
use crate::models::music::{
    AlbumID3Response, ArtistID3Response, ChildResponse, NowPlayingEntryResponse,
    NowPlayingResponse, Starred2Response, saturating_i64_to_i32,
};

/// Query parameters for star/unstar.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
#[expect(
    clippy::struct_field_names,
    reason = "Subsonic API parameter names all end in 'Id'"
)]
pub struct StarParams {
    #[serde(rename = "artistId")]
    artist_id: Vec<String>,
    #[serde(rename = "albumId")]
    album_id: Vec<String>,
    #[serde(rename = "id")]
    song_id: Vec<String>,
}

/// GET/POST /rest/star[.view]
///
/// Stars one or more artists, albums, or songs.
/// Supports multiple IDs via repeated parameters: `?id=1&id=2&albumId=3`
pub async fn star(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<StarParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    if !auth.user.roles.comment_role {
        return util::unauthorized(&auth);
    }

    let user_id = auth.user.id;

    for artist_id in &params.artist_id {
        let Some(artist_id) = crate::models::music::EntityId::parse_artist(artist_id) else {
            return util::service_error(&auth, format!("Invalid artistId: {artist_id}"));
        };
        if let Err(error) = auth.music().star_artist(user_id, artist_id) {
            return util::service_error(&auth, error);
        }
    }
    for album_id in &params.album_id {
        let Some(album_id) = crate::models::music::EntityId::parse_album(album_id) else {
            return util::service_error(&auth, format!("Invalid albumId: {album_id}"));
        };
        if let Err(error) = auth.music().star_album(user_id, album_id) {
            return util::service_error(&auth, error);
        }
    }
    for song_id in &params.song_id {
        let Some(song_id) = crate::models::music::EntityId::parse_song(song_id) else {
            return util::service_error(&auth, format!("Invalid id: {song_id}"));
        };
        if let Err(error) = auth.music().star_song(user_id, song_id) {
            return util::service_error(&auth, error);
        }
    }

    SubsonicResponse::empty(auth.format).into_response()
}

/// GET/POST /rest/unstar[.view]
///
/// Unstars one or more artists, albums, or songs.
/// Supports multiple IDs via repeated parameters: `?id=1&id=2&albumId=3`
pub async fn unstar(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<StarParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    if !auth.user.roles.comment_role {
        return util::unauthorized(&auth);
    }

    let user_id = auth.user.id;

    for artist_id in &params.artist_id {
        let Some(artist_id) = crate::models::music::EntityId::parse_artist(artist_id) else {
            return util::service_error(&auth, format!("Invalid artistId: {artist_id}"));
        };
        if let Err(error) = auth.music().unstar_artist(user_id, artist_id) {
            return util::service_error(&auth, error);
        }
    }
    for album_id in &params.album_id {
        let Some(album_id) = crate::models::music::EntityId::parse_album(album_id) else {
            return util::service_error(&auth, format!("Invalid albumId: {album_id}"));
        };
        if let Err(error) = auth.music().unstar_album(user_id, album_id) {
            return util::service_error(&auth, error);
        }
    }
    for song_id in &params.song_id {
        let Some(song_id) = crate::models::music::EntityId::parse_song(song_id) else {
            return util::service_error(&auth, format!("Invalid id: {song_id}"));
        };
        if let Err(error) = auth.music().unstar_song(user_id, song_id) {
            return util::service_error(&auth, error);
        }
    }

    SubsonicResponse::empty(auth.format).into_response()
}

/// GET/POST /rest/getStarred2[.view]
///
/// Returns all starred artists, albums, and songs for the current user.
/// Uses ID3 tags (artist/album/song structure).
pub async fn get_starred2(auth: SubsonicContext) -> impl IntoResponse {
    let user_id = auth.user.id;

    let starred_artists = match auth.music().get_starred_artists(user_id) {
        Ok(v) => v,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };
    let artist_ids: Vec<i32> = starred_artists.iter().map(|(a, _)| a.id).collect();
    let album_counts = match auth.music().get_artist_album_counts_batch(&artist_ids) {
        Ok(v) => v,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    let artists: Vec<ArtistID3Response> = starred_artists
        .iter()
        .map(|(artist, starred_at)| {
            let album_count = album_counts.get(&artist.id).copied().unwrap_or(0);
            ArtistID3Response::from_artist_with_starred(
                artist,
                Some(saturating_i64_to_i32(album_count)),
                Some(starred_at),
            )
        })
        .collect();

    let starred_albums = match auth.music().get_starred_albums(user_id) {
        Ok(v) => v,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };
    let album_ids: Vec<i32> = starred_albums.iter().map(|(album, _)| album.id).collect();
    let album_annotations = match auth
        .music()
        .get_album_annotations_batch(user_id, &album_ids)
    {
        Ok(v) => v,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };
    let albums: Vec<AlbumID3Response> = starred_albums
        .iter()
        .map(|(album, starred_at)| {
            AlbumID3Response::from_album_with_starred(album, Some(starred_at))
                .with_annotations(album_annotations.get(&album.id))
        })
        .collect();

    let starred_songs = match auth.music().get_starred_songs(user_id) {
        Ok(v) => v,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };
    let song_ids: Vec<i32> = starred_songs.iter().map(|(song, _)| song.id).collect();
    let song_annotations = match auth.music().get_song_annotations_batch(user_id, &song_ids) {
        Ok(v) => v,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };
    let songs: Vec<ChildResponse> = starred_songs
        .iter()
        .map(|(song, starred_at)| {
            ChildResponse::from_song_with_starred(song, Some(starred_at))
                .with_annotations(song_annotations.get(&song.id))
        })
        .collect();

    let response = Starred2Response {
        artists,
        albums,
        songs,
    };
    SubsonicResponse::starred2(auth.format, response).into_response()
}

/// Query parameters for scrobble.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ScrobbleParams {
    #[serde(rename = "id")]
    song_id: Vec<String>,
    time: Vec<i64>,
    submission: Option<String>,
}

/// GET/POST /rest/scrobble[.view]
///
/// Registers the local playback of one or more media files.
/// Typically used to notify the server about what is currently being played locally.
///
/// Parameters:
/// - `id` (required): The ID of the song being played (can be repeated)
/// - `time` (optional): Time in milliseconds since the Unix epoch (can be repeated, one per id)
/// - `submission` (optional): Whether this is a "scrobble" (true) or a "now playing" notification (false). Default true.
pub async fn scrobble(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<ScrobbleParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let user_id = auth.user.id;

    let submission = params
        .submission
        .as_deref()
        .is_none_or(|s| s != "false" && s != "0");

    let player_id = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    for (i, song_id) in params.song_id.iter().enumerate() {
        let Some(song_id) = crate::models::music::EntityId::parse_song(song_id) else {
            return util::service_error(&auth, format!("Invalid id: {song_id}"));
        };
        let time = params.time.get(i).copied();

        if let Err(error) = auth.music().scrobble(user_id, song_id, time, submission) {
            return util::service_error(&auth, error);
        }

        if !submission && let Err(error) = auth.music().set_now_playing(user_id, song_id, player_id)
        {
            return util::service_error(&auth, error);
        }
    }

    SubsonicResponse::empty(auth.format).into_response()
}

/// Query parameters for reportPlayback.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct ReportPlaybackParams {
    /// ID of the song being played.
    pub media_id: Option<String>,
    /// Media type. Only "song" is supported.
    pub media_type: Option<String>,
    /// Current playback position in milliseconds.
    pub position_ms: Option<i64>,
    /// Playback state: "playing", "paused", or "stopped".
    pub state: Option<String>,
    /// Playback rate multiplier (informational only).
    pub playback_rate: Option<f64>,
    /// Whether to skip recording a scrobble on stop.
    pub ignore_scrobble: Option<bool>,
}

/// Whether a stopped playback report should count as a scrobble.
///
/// Mirrors navidrome's rule: the track must have been played for at least
/// 50% of its duration, capped at 4 minutes.
fn scrobble_threshold_met(position_ms: i64, duration_secs: i32) -> bool {
    let duration_ms = i64::from(duration_secs.max(0)) * 1000;
    let threshold = (duration_ms / 2).min(240_000);
    position_ms >= threshold
}

/// GET/POST /rest/reportPlayback[.view]
///
/// Reports playback progress for a song (`OpenSubsonic` playbackReport extension).
/// "playing" registers a now-playing entry; "stopped" records a scrobble
/// unless `ignoreScrobble` is set or the 50%/4-minute threshold isn't met.
pub async fn report_playback(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<ReportPlaybackParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    let Some(song_id) = params
        .media_id
        .as_deref()
        .and_then(crate::models::music::EntityId::parse_song)
    else {
        return util::missing_param(&auth, "mediaId");
    };
    let Some(media_type) = params.media_type.as_deref() else {
        return util::missing_param(&auth, "mediaType");
    };
    if media_type != "song" {
        return util::service_error(&auth, format!("Unsupported mediaType: {media_type}"));
    }
    let Some(position_ms) = params.position_ms else {
        return util::missing_param(&auth, "positionMs");
    };
    if position_ms < 0 {
        return util::service_error(&auth, "positionMs must be non-negative");
    }
    let Some(state) = params.state.as_deref() else {
        return util::missing_param(&auth, "state");
    };
    if let Some(rate) = params.playback_rate
        && (!rate.is_finite() || rate <= 0.0)
    {
        return util::service_error(&auth, "playbackRate must be a finite positive number");
    }

    let user_id = auth.user.id;
    let player_id = if auth.params.c.is_empty() {
        None
    } else {
        Some(auth.params.c.as_str())
    };

    match state {
        "playing" => {
            if let Err(error) = auth.music().set_now_playing(user_id, song_id, player_id) {
                return util::service_error(&auth, error);
            }
        }
        "paused" => {}
        "stopped" => {
            let duration_secs = match auth.music().get_song(song_id) {
                Ok(Some(song)) => song.duration,
                Ok(None) => return util::not_found(&auth, "Song"),
                Err(error) => return util::service_error(&auth, error),
            };

            if !params.ignore_scrobble.unwrap_or(false)
                && scrobble_threshold_met(position_ms, duration_secs)
                && let Err(error) = auth.music().scrobble(user_id, song_id, None, true)
            {
                return util::service_error(&auth, error);
            }
        }
        _ => {
            return util::service_error(&auth, format!("Invalid state: {state}"));
        }
    }

    SubsonicResponse::empty(auth.format).into_response()
}

/// GET/POST /rest/getNowPlaying[.view]
///
/// Returns what is currently being played by all users.
pub async fn get_now_playing(auth: SubsonicContext) -> impl IntoResponse {
    let entries = match auth.music().get_now_playing() {
        Ok(v) => v,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    let entry_responses: Vec<NowPlayingEntryResponse> = entries
        .iter()
        .map(|entry| {
            NowPlayingEntryResponse::from_now_playing(
                &entry.song,
                entry.username.clone(),
                entry.minutes_ago,
                entry.player_id.clone(),
            )
        })
        .collect();

    let response = NowPlayingResponse {
        entries: entry_responses,
    };

    SubsonicResponse::now_playing(auth.format, response).into_response()
}

// ============================================================================
// Rating endpoints
// ============================================================================

/// Query parameters for setRating.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct SetRatingParams {
    /// The ID of the item (song, album, or artist) to rate.
    pub id: Option<String>,
    /// The rating (0-5). 0 removes the rating.
    pub rating: Option<i32>,
}

/// GET/POST /rest/setRating[.view]
///
/// Sets the rating for a music file (song).
///
/// Parameters:
/// - `id` (required): The ID of the item to rate
/// - `rating` (required): The rating (0-5). 0 removes the rating.
pub async fn set_rating(
    crate::api::auth::SubsonicQuery(params): crate::api::auth::SubsonicQuery<SetRatingParams>,
    auth: SubsonicContext,
) -> impl IntoResponse {
    if !auth.user.roles.comment_role {
        return util::unauthorized(&auth);
    }

    let Some(id_str) = params.id.as_ref() else {
        return util::missing_param(&auth, "id");
    };
    // Bare integers are song ids; al-/ar- prefixes rate albums/artists
    let Some(entity_id) = crate::models::music::EntityId::parse(id_str) else {
        return util::service_error(&auth, format!("Invalid id: {id_str}"));
    };

    let rating = match params.rating {
        Some(r) if (0..=5).contains(&r) => r,
        Some(_) => {
            return util::service_error(&auth, "Rating must be between 0 and 5");
        }
        None => {
            return util::missing_param(&auth, "rating");
        }
    };

    let user_id = auth.user.id;

    let result = match entity_id {
        crate::models::music::EntityId::Song(id) => {
            auth.music().set_song_rating(user_id, id, rating)
        }
        crate::models::music::EntityId::Album(id) => {
            auth.music().set_album_rating(user_id, id, rating)
        }
        crate::models::music::EntityId::Artist(id) => {
            auth.music().set_artist_rating(user_id, id, rating)
        }
        crate::models::music::EntityId::Playlist(_) => {
            return util::service_error(&auth, format!("Invalid id: {id_str}"));
        }
    };

    match result {
        Ok(()) => SubsonicResponse::empty(auth.format).into_response(),
        Err(error) => util::service_error(&auth, error),
    }
}

#[cfg(test)]
mod tests {
    use super::{ScrobbleParams, StarParams, scrobble_threshold_met};

    #[test]
    fn vec_params_accept_single_and_repeated_values() {
        // Subsonic clients send multi-value params both ways; the query
        // deserializer must accept a single value and repeated keys.
        let single: StarParams = serde_html_form::from_str("id=1").expect("single id parses");
        assert_eq!(single.song_id, vec!["1".to_string()]);

        let repeated: StarParams =
            serde_html_form::from_str("id=1&id=2&albumId=3").expect("repeated ids parse");
        assert_eq!(repeated.song_id, vec!["1".to_string(), "2".to_string()]);
        assert_eq!(repeated.album_id, vec!["3".to_string()]);

        let scrobble: ScrobbleParams =
            serde_html_form::from_str("id=7&time=1000&id=8&time=2000").expect("pairs parse");
        assert_eq!(scrobble.song_id, vec!["7".to_string(), "8".to_string()]);
        assert_eq!(scrobble.time, vec![1000, 2000]);
    }

    #[test]
    fn scrobble_threshold_requires_half_the_track() {
        // 4-minute track: threshold is 2 minutes (50%)
        assert!(!scrobble_threshold_met(119_999, 240));
        assert!(scrobble_threshold_met(120_000, 240));
    }

    #[test]
    fn scrobble_threshold_caps_at_four_minutes() {
        // 20-minute track: threshold is the 4-minute cap, not 10 minutes
        assert!(!scrobble_threshold_met(239_999, 1200));
        assert!(scrobble_threshold_met(240_000, 1200));
    }

    #[test]
    fn scrobble_threshold_handles_zero_duration() {
        assert!(scrobble_threshold_met(0, 0));
        assert!(!scrobble_threshold_met(-1, 0));
    }
}
