//! Artist info cache repository for Last.fm data.

use diesel::prelude::*;

use crate::db::DbPool;
use crate::db::repo::error::UserRepoError;
use crate::lastfm::models::LastFmArtistCache;
use chrono::NaiveDateTime;

/// Cache TTL in hours (7 days)
const CACHE_TTL_HOURS: i64 = 7 * 24;

/// Repository for artist info cache operations.
#[derive(Clone, Debug)]
pub struct ArtistInfoCacheRepository {
    pool: DbPool,
}

impl ArtistInfoCacheRepository {
    /// Create a new artist info cache repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get cached artist info if it exists and is not expired.
    pub fn get_cached(
        &self,
        target_artist_id: i32,
    ) -> Result<Option<LastFmArtistCache>, UserRepoError> {
        use crate::db::schema::artist_lastfm_info::dsl::{
            artist_id, artist_lastfm_info, biography, large_image_url, last_fm_url,
            medium_image_url, similar_artists, small_image_url, updated_at,
        };

        let mut conn = self.pool.get()?;

        let result = artist_lastfm_info
            .filter(artist_id.eq(target_artist_id))
            .select((
                artist_id,
                biography,
                last_fm_url,
                small_image_url,
                medium_image_url,
                large_image_url,
                similar_artists,
                updated_at,
            ))
            .first::<(
                i32,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                Option<String>,
                NaiveDateTime,
            )>(&mut conn)
            .optional()?;

        Ok(result.map(|row| LastFmArtistCache {
            artist_id: row.0,
            biography: row.1,
            last_fm_url: row.2,
            small_image_url: row.3,
            medium_image_url: row.4,
            large_image_url: row.5,
            similar_artists: row
                .6
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default(),
            updated_at: row.7,
        }))
    }

    /// Check if cached data is expired.
    #[must_use]
    pub fn is_cache_expired(&self, cache: &LastFmArtistCache) -> bool {
        let now = chrono::Local::now().naive_local();
        let age = now.signed_duration_since(cache.updated_at);
        age.num_hours() > CACHE_TTL_HOURS
    }

    /// Get artist info, returning None if cache is expired or missing.
    pub fn get_valid_cache(
        &self,
        target_artist_id: i32,
    ) -> Result<Option<LastFmArtistCache>, UserRepoError> {
        match self.get_cached(target_artist_id)? {
            Some(cache) if !self.is_cache_expired(&cache) => Ok(Some(cache)),
            _ => Ok(None),
        }
    }

    /// Save or update artist info cache.
    pub fn save_cache(&self, cache: &LastFmArtistCache) -> Result<(), UserRepoError> {
        use crate::db::schema::artist_lastfm_info::dsl::{
            artist_id, artist_lastfm_info, biography, large_image_url, last_fm_url,
            medium_image_url, similar_artists, small_image_url, updated_at,
        };

        let mut conn = self.pool.get()?;

        let similar_json = serde_json::to_string(&cache.similar_artists).unwrap_or_default();

        diesel::replace_into(artist_lastfm_info)
            .values((
                artist_id.eq(cache.artist_id),
                biography.eq(&cache.biography),
                last_fm_url.eq(&cache.last_fm_url),
                small_image_url.eq(&cache.small_image_url),
                medium_image_url.eq(&cache.medium_image_url),
                large_image_url.eq(&cache.large_image_url),
                similar_artists.eq(&similar_json),
                updated_at.eq(chrono::Local::now().naive_local()),
            ))
            .execute(&mut conn)?;

        Ok(())
    }

    /// Clear cache for a specific artist.
    pub fn clear_cache(&self, target_artist_id: i32) -> Result<bool, UserRepoError> {
        use crate::db::schema::artist_lastfm_info::dsl::{artist_id, artist_lastfm_info};

        let mut conn = self.pool.get()?;

        let deleted = diesel::delete(artist_lastfm_info.filter(artist_id.eq(target_artist_id)))
            .execute(&mut conn)?;

        Ok(deleted > 0)
    }

    /// Clear all expired cache entries.
    pub fn clear_expired(&self) -> Result<usize, UserRepoError> {
        use crate::db::schema::artist_lastfm_info::dsl::{artist_lastfm_info, updated_at};

        let mut conn = self.pool.get()?;
        let cutoff = chrono::Local::now().naive_local() - chrono::Duration::hours(CACHE_TTL_HOURS);

        let deleted =
            diesel::delete(artist_lastfm_info.filter(updated_at.lt(cutoff))).execute(&mut conn)?;

        Ok(deleted)
    }
}
