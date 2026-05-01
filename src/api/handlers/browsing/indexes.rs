//! Index and structure browsing handlers.

use std::collections::BTreeMap;

use axum::response::IntoResponse;

use crate::api::auth::SubsonicContext;
use crate::api::handlers::util;
use crate::api::response::SubsonicResponse;
use crate::models::music::{
    Artist, ArtistID3Response, ArtistResponse, ArtistsID3Response, IndexID3Response, IndexResponse,
    IndexesResponse, MusicFolderResponse, saturating_i64_to_i32,
};

fn artist_index_key(artist: &Artist) -> String {
    let first_char = artist
        .sort_name
        .as_ref()
        .unwrap_or(&artist.name)
        .chars()
        .next()
        .unwrap_or('#')
        .to_uppercase()
        .next()
        .unwrap_or('#');

    if first_char.is_alphabetic() {
        first_char.to_string()
    } else {
        "#".to_string()
    }
}

/// GET/POST /rest/getMusicFolders[.view]
///
/// Returns all configured top-level music folders.
pub async fn get_music_folders(auth: SubsonicContext) -> impl IntoResponse {
    let folders = match auth.music().get_music_folders() {
        Ok(folders) => folders,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };
    let responses: Vec<MusicFolderResponse> =
        folders.iter().map(MusicFolderResponse::from).collect();
    SubsonicResponse::music_folders(auth.format, responses).into_response()
}

/// GET/POST /rest/getIndexes[.view]
///
/// Returns an indexed structure of all artists.
/// This is used by older clients that use the folder-based browsing model.
pub async fn get_indexes(auth: SubsonicContext) -> impl IntoResponse {
    let artists = match auth.music().get_artists() {
        Ok(artists) => artists,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };
    let user_id = auth.user.id;

    // Get starred status for all artists in a single batch query
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let starred_map = match auth
        .music()
        .get_starred_at_for_artists_batch(user_id, &artist_ids)
    {
        Ok(starred_map) => starred_map,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    // Group artists by first letter
    let mut index_map: BTreeMap<String, Vec<ArtistResponse>> = BTreeMap::new();

    for artist in &artists {
        let starred_at = starred_map.get(&artist.id);

        index_map
            .entry(artist_index_key(artist))
            .or_default()
            .push(ArtistResponse::from_artist_with_starred(artist, starred_at));
    }

    // Convert to response format
    let indexes: Vec<IndexResponse> = index_map
        .into_iter()
        .map(|(name, artists)| IndexResponse { name, artists })
        .collect();

    // Get last modified time (using current timestamp for now)
    let last_modified = match auth.music().get_artists_last_modified() {
        Ok(value) => value.map_or(0, |dt| dt.and_utc().timestamp_millis()),
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    let response = IndexesResponse {
        ignored_articles: "The El La Los Las Le Les".to_string(),
        last_modified,
        indexes,
    };

    SubsonicResponse::indexes(auth.format, response).into_response()
}

/// GET/POST /rest/getArtists[.view]
///
/// Similar to getIndexes, but returns artists using ID3 tags.
/// This is the preferred endpoint for modern clients.
pub async fn get_artists(auth: SubsonicContext) -> impl IntoResponse {
    let artists = match auth.music().get_artists() {
        Ok(artists) => artists,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };
    let user_id = auth.user.id;

    // Get album counts for all artists in a single batch query
    let artist_ids: Vec<i32> = artists.iter().map(|a| a.id).collect();
    let album_counts = match auth.music().get_artist_album_counts_batch(&artist_ids) {
        Ok(album_counts) => album_counts,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    // Get starred status for all artists in a single batch query
    let starred_map = match auth
        .music()
        .get_starred_at_for_artists_batch(user_id, &artist_ids)
    {
        Ok(starred_map) => starred_map,
        Err(e) => {
            return util::service_error(&auth, e);
        }
    };

    // Group artists by first letter
    let mut index_map: BTreeMap<String, Vec<ArtistID3Response>> = BTreeMap::new();

    for artist in &artists {
        // Get album count and starred status from batch results
        let album_count = album_counts.get(&artist.id).copied().unwrap_or(0);
        let starred_at = starred_map.get(&artist.id);

        index_map.entry(artist_index_key(artist)).or_default().push(
            ArtistID3Response::from_artist_with_starred(
                artist,
                Some(saturating_i64_to_i32(album_count)),
                starred_at,
            ),
        );
    }

    // Convert to response format
    let indexes: Vec<IndexID3Response> = index_map
        .into_iter()
        .map(|(name, artists)| IndexID3Response { name, artists })
        .collect();

    let response = ArtistsID3Response {
        ignored_articles: "The El La Los Las Le Les".to_string(),
        indexes,
    };

    SubsonicResponse::artists(auth.format, response).into_response()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::artist_index_key;
    use crate::models::music::Artist;

    fn artist(name: &str, sort_name: Option<&str>) -> Artist {
        let now = NaiveDate::from_ymd_opt(2024, 1, 2)
            .expect("valid date")
            .and_hms_opt(3, 4, 5)
            .expect("valid time");

        Artist {
            id: 1,
            name: name.to_string(),
            sort_name: sort_name.map(str::to_string),
            musicbrainz_id: None,
            cover_art: None,
            artist_image_url: None,
            created_at: now,
            updated_at: now,
        }
    }

    #[test]
    fn artist_index_key_prefers_sort_name_and_uppercases_first_letter() {
        assert_eq!(
            artist_index_key(&artist("The Beatles", Some("beatles"))),
            "B"
        );
    }

    #[test]
    fn artist_index_key_groups_non_alphabetic_and_empty_names_under_hash() {
        assert_eq!(artist_index_key(&artist("123 Go", None)), "#");
        assert_eq!(artist_index_key(&artist("", None)), "#");
    }
}
