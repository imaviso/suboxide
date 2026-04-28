//! API services coordinating repositories and external APIs.

use std::collections::HashMap;

use chrono::NaiveDateTime;

use crate::crypto::password::hash_password;
use crate::db::{
    AlbumRepository, ArtistInfoCacheRepository, ArtistRepository, DbPool, MusicFolderRepository,
    MusicRepoError, MusicRepoErrorKind, NewUser, NowPlayingEntry, NowPlayingRepository, PlayQueue,
    PlayQueueRepository, PlaylistRepository, RatingRepository, RemoteCommand,
    RemoteControlRepository, RemoteSession, RemoteState, ScrobbleRepository, SongRepository,
    StarredRepository, UserRepoError, UserRepoErrorKind, UserRepository, UserUpdate,
};
use crate::lastfm::{LastFmClient, models::extract_biography, models::extract_image_urls};
use crate::models::User;
use crate::models::music::{
    Album, Artist, ArtistID3Response, ArtistInfo2Response, Song, saturating_i64_to_i32,
};
use crate::models::user::UserRoles;
use crate::paths::resolve_cover_art_dir;
use crate::scanner::lyrics::{ExtractedLyrics, extract_lyrics};

const LASTFM_PLACEHOLDER_IMAGE_MARKER: &str = "2a96cbd8b46e442fc41c2b86b821562f";

#[derive(Debug, thiserror::Error)]
enum ArtistImageError {
    #[error("artist image IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("artist image HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("artist image request returned {0}")]
    HttpStatus(reqwest::StatusCode),
}

fn is_lastfm_placeholder_image(url: &str) -> bool {
    url.contains(LASTFM_PLACEHOLDER_IMAGE_MARKER)
}

async fn download_artist_image(
    artist_id: i32,
    image_url: &str,
) -> Result<String, ArtistImageError> {
    use tokio::io::AsyncWriteExt;

    let cover_art_dir = resolve_cover_art_dir();
    tokio::fs::create_dir_all(&cover_art_dir).await?;

    let image_extension = std::path::Path::new(image_url).extension();
    let extension =
        if image_extension.is_some_and(|extension| extension.eq_ignore_ascii_case("png")) {
            "png"
        } else if image_extension.is_some_and(|extension| extension.eq_ignore_ascii_case("gif")) {
            "gif"
        } else {
            "jpg"
        };

    let cover_art_id = format!("artist-{artist_id}");
    let filepath = cover_art_dir.join(format!("{cover_art_id}.{extension}"));
    if tokio::fs::try_exists(&filepath).await? {
        return Ok(cover_art_id);
    }

    let response = reqwest::get(image_url).await?;
    if !response.status().is_success() {
        return Err(ArtistImageError::HttpStatus(response.status()));
    }

    let bytes = response.bytes().await?;
    let mut file = tokio::fs::File::create(&filepath).await?;
    file.write_all(&bytes).await?;

    Ok(cover_art_id)
}

/// Music library and playback operations.
#[derive(Clone, Debug)]
pub struct MusicLibrary {
    pool: DbPool,
    lastfm_client: LastFmClient,
}

impl MusicLibrary {
    /// Create a new music library.
    #[must_use]
    pub const fn new(pool: DbPool, lastfm_client: LastFmClient) -> Self {
        Self {
            pool,
            lastfm_client,
        }
    }

    // ========================================================================
    // Music folders
    // ========================================================================

    pub(in crate::api) fn get_music_folders(
        &self,
    ) -> Result<Vec<crate::models::music::MusicFolder>, MusicRepoError> {
        MusicFolderRepository::new(self.pool.clone()).find_enabled()
    }

    // ========================================================================
    // Artists
    // ========================================================================

    pub(in crate::api) fn get_artists(&self) -> Result<Vec<Artist>, MusicRepoError> {
        ArtistRepository::new(self.pool.clone()).find_all()
    }

    pub(in crate::api) fn get_artists_by_music_folder(
        &self,
        folder_id: i32,
    ) -> Result<Vec<Artist>, MusicRepoError> {
        ArtistRepository::new(self.pool.clone()).find_by_music_folder(folder_id)
    }

    pub(in crate::api) fn get_artists_last_modified(
        &self,
    ) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        ArtistRepository::new(self.pool.clone()).get_last_modified()
    }

    pub(in crate::api) fn get_artist_album_count(
        &self,
        artist_id: i32,
    ) -> Result<i64, MusicRepoError> {
        ArtistRepository::new(self.pool.clone()).count_albums(artist_id)
    }

    pub(in crate::api) fn get_artist(
        &self,
        artist_id: i32,
    ) -> Result<Option<Artist>, MusicRepoError> {
        ArtistRepository::new(self.pool.clone()).find_by_id(artist_id)
    }

    pub(in crate::api) fn get_artist_album_counts_batch(
        &self,
        artist_ids: &[i32],
    ) -> Result<HashMap<i32, i64>, MusicRepoError> {
        ArtistRepository::new(self.pool.clone()).count_albums_batch(artist_ids)
    }

    // ========================================================================
    // Albums
    // ========================================================================

    pub(in crate::api) fn get_album(&self, album_id: i32) -> Result<Option<Album>, MusicRepoError> {
        AlbumRepository::new(self.pool.clone()).find_by_id(album_id)
    }

    pub(in crate::api) fn get_albums_by_artist(
        &self,
        artist_id: i32,
    ) -> Result<Vec<Album>, MusicRepoError> {
        AlbumRepository::new(self.pool.clone()).find_by_artist(artist_id)
    }

    pub(in crate::api) fn get_albums_alphabetical_by_name(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        AlbumRepository::new(self.pool.clone()).find_alphabetical_by_name(offset, limit)
    }

    pub(in crate::api) fn get_albums_alphabetical_by_artist(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        AlbumRepository::new(self.pool.clone()).find_alphabetical_by_artist(offset, limit)
    }

    pub(in crate::api) fn get_albums_newest(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        AlbumRepository::new(self.pool.clone()).find_newest(offset, limit)
    }

    pub(in crate::api) fn get_albums_frequent(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        AlbumRepository::new(self.pool.clone()).find_frequent(offset, limit)
    }

    pub(in crate::api) fn get_albums_recent(
        &self,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        AlbumRepository::new(self.pool.clone()).find_recent(offset, limit)
    }

    pub(in crate::api) fn get_albums_random(
        &self,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        AlbumRepository::new(self.pool.clone()).find_random(limit)
    }

    pub(in crate::api) fn get_albums_by_year(
        &self,
        from_year: i32,
        to_year: i32,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        AlbumRepository::new(self.pool.clone())
            .find_by_year_range(from_year, to_year, offset, limit)
    }

    pub(in crate::api) fn get_albums_by_genre(
        &self,
        genre: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        AlbumRepository::new(self.pool.clone()).find_by_genre(genre, offset, limit)
    }

    pub(in crate::api) fn get_albums_starred(
        &self,
        user_id: i32,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        let starred = StarredRepository::new(self.pool.clone())
            .get_starred_albums_paginated(user_id, offset, limit)?;
        Ok(starred.into_iter().map(|(album, _)| album).collect())
    }

    pub(in crate::api) fn get_albums_highest(
        &self,
        user_id: i32,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        let album_ids = RatingRepository::new(self.pool.clone())
            .get_highest_rated_album_ids(user_id, offset, limit)?;

        if album_ids.is_empty() {
            return Ok(vec![]);
        }

        let albums = AlbumRepository::new(self.pool.clone()).find_by_ids(&album_ids)?;
        let mut album_map: HashMap<i32, Album> = albums.into_iter().map(|a| (a.id, a)).collect();

        Ok(album_ids
            .into_iter()
            .filter_map(|id| album_map.remove(&id))
            .collect())
    }

    // ========================================================================
    // Songs
    // ========================================================================

    pub(in crate::api) fn get_song(&self, song_id: i32) -> Result<Option<Song>, MusicRepoError> {
        SongRepository::new(self.pool.clone()).find_by_id(song_id)
    }

    pub(in crate::api) fn find_song_by_artist_and_title(
        &self,
        artist: &str,
        title: &str,
    ) -> Result<Option<Song>, MusicRepoError> {
        SongRepository::new(self.pool.clone()).find_by_artist_and_title(artist, title)
    }

    pub(in crate::api) fn get_songs_by_album(
        &self,
        album_id: i32,
    ) -> Result<Vec<Song>, MusicRepoError> {
        SongRepository::new(self.pool.clone()).find_by_album(album_id)
    }

    pub(in crate::api) fn get_random_songs(
        &self,
        size: i64,
        genre: Option<&str>,
        from_year: Option<i32>,
        to_year: Option<i32>,
        music_folder_id: Option<i32>,
    ) -> Result<Vec<Song>, MusicRepoError> {
        SongRepository::new(self.pool.clone()).find_random(
            size,
            genre,
            from_year,
            to_year,
            music_folder_id,
        )
    }

    pub(in crate::api) fn get_songs_by_genre(
        &self,
        genre: &str,
        count: i64,
        offset: i64,
        music_folder_id: Option<i32>,
    ) -> Result<Vec<Song>, MusicRepoError> {
        SongRepository::new(self.pool.clone()).find_by_genre(genre, count, offset, music_folder_id)
    }

    pub(in crate::api) fn get_similar_songs_by_artist(
        &self,
        artist_id: i32,
        exclude_song_id: i32,
        limit: i64,
    ) -> Result<Vec<Song>, MusicRepoError> {
        SongRepository::new(self.pool.clone()).find_random_by_artist(
            artist_id,
            exclude_song_id,
            limit,
        )
    }

    pub(in crate::api) fn get_top_songs_by_artist_name(
        &self,
        artist_name: &str,
        limit: i64,
    ) -> Result<Vec<Song>, MusicRepoError> {
        SongRepository::new(self.pool.clone()).find_top_by_artist_name(artist_name, limit)
    }

    pub(in crate::api) fn get_song_lyrics(
        &self,
        song_id: i32,
    ) -> Result<Vec<ExtractedLyrics>, MusicRepoError> {
        let Some(song) = self.get_song(song_id)? else {
            return Ok(Vec::new());
        };
        Ok(extract_lyrics(std::path::Path::new(&song.path)))
    }

    // ========================================================================
    // Genres & search
    // ========================================================================

    pub(in crate::api) fn get_genres(&self) -> Result<Vec<(String, i64, i64)>, MusicRepoError> {
        SongRepository::new(self.pool.clone()).get_genres()
    }

    pub(in crate::api) fn search_artists(
        &self,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Artist>, MusicRepoError> {
        ArtistRepository::new(self.pool.clone()).search(query, offset, limit)
    }

    pub(in crate::api) fn search_albums(
        &self,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Album>, MusicRepoError> {
        AlbumRepository::new(self.pool.clone()).search(query, offset, limit)
    }

    pub(in crate::api) fn search_songs(
        &self,
        query: &str,
        offset: i64,
        limit: i64,
    ) -> Result<Vec<Song>, MusicRepoError> {
        SongRepository::new(self.pool.clone()).search(query, offset, limit)
    }

    // ========================================================================
    // Starred
    // ========================================================================

    pub(in crate::api) fn star_artist(
        &self,
        user_id: i32,
        artist_id: i32,
    ) -> Result<(), MusicRepoError> {
        StarredRepository::new(self.pool.clone()).star_artist(user_id, artist_id)
    }

    pub(in crate::api) fn star_album(
        &self,
        user_id: i32,
        album_id: i32,
    ) -> Result<(), MusicRepoError> {
        StarredRepository::new(self.pool.clone()).star_album(user_id, album_id)
    }

    pub(in crate::api) fn star_song(
        &self,
        user_id: i32,
        song_id: i32,
    ) -> Result<(), MusicRepoError> {
        StarredRepository::new(self.pool.clone()).star_song(user_id, song_id)
    }

    pub(in crate::api) fn unstar_artist(
        &self,
        user_id: i32,
        artist_id: i32,
    ) -> Result<bool, MusicRepoError> {
        StarredRepository::new(self.pool.clone()).unstar_artist(user_id, artist_id)
    }

    pub(in crate::api) fn unstar_album(
        &self,
        user_id: i32,
        album_id: i32,
    ) -> Result<bool, MusicRepoError> {
        StarredRepository::new(self.pool.clone()).unstar_album(user_id, album_id)
    }

    pub(in crate::api) fn unstar_song(
        &self,
        user_id: i32,
        song_id: i32,
    ) -> Result<bool, MusicRepoError> {
        StarredRepository::new(self.pool.clone()).unstar_song(user_id, song_id)
    }

    pub(in crate::api) fn get_starred_artists(
        &self,
        user_id: i32,
    ) -> Result<Vec<(Artist, NaiveDateTime)>, MusicRepoError> {
        StarredRepository::new(self.pool.clone()).get_starred_artists(user_id)
    }

    pub(in crate::api) fn get_starred_albums(
        &self,
        user_id: i32,
    ) -> Result<Vec<(Album, NaiveDateTime)>, MusicRepoError> {
        StarredRepository::new(self.pool.clone()).get_starred_albums(user_id)
    }

    pub(in crate::api) fn get_starred_songs(
        &self,
        user_id: i32,
    ) -> Result<Vec<(Song, NaiveDateTime)>, MusicRepoError> {
        StarredRepository::new(self.pool.clone()).get_starred_songs(user_id)
    }

    pub(in crate::api) fn get_starred_at_for_artist(
        &self,
        user_id: i32,
        artist_id: i32,
    ) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        StarredRepository::new(self.pool.clone()).get_starred_at_for_artist(user_id, artist_id)
    }

    pub(in crate::api) fn get_starred_at_for_album(
        &self,
        user_id: i32,
        album_id: i32,
    ) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        StarredRepository::new(self.pool.clone()).get_starred_at_for_album(user_id, album_id)
    }

    pub(in crate::api) fn get_starred_at_for_song(
        &self,
        user_id: i32,
        song_id: i32,
    ) -> Result<Option<NaiveDateTime>, MusicRepoError> {
        StarredRepository::new(self.pool.clone()).get_starred_at_for_song(user_id, song_id)
    }

    pub(in crate::api) fn get_starred_at_for_songs_batch(
        &self,
        user_id: i32,
        song_ids: &[i32],
    ) -> Result<HashMap<i32, NaiveDateTime>, MusicRepoError> {
        StarredRepository::new(self.pool.clone()).get_starred_at_for_songs_batch(user_id, song_ids)
    }

    pub(in crate::api) fn get_starred_at_for_albums_batch(
        &self,
        user_id: i32,
        album_ids: &[i32],
    ) -> Result<HashMap<i32, NaiveDateTime>, MusicRepoError> {
        StarredRepository::new(self.pool.clone())
            .get_starred_at_for_albums_batch(user_id, album_ids)
    }

    pub(in crate::api) fn get_starred_at_for_artists_batch(
        &self,
        user_id: i32,
        artist_ids: &[i32],
    ) -> Result<HashMap<i32, NaiveDateTime>, MusicRepoError> {
        StarredRepository::new(self.pool.clone())
            .get_starred_at_for_artists_batch(user_id, artist_ids)
    }

    // ========================================================================
    // Scrobble & now playing
    // ========================================================================

    pub(in crate::api) fn scrobble(
        &self,
        user_id: i32,
        song_id: i32,
        time: Option<i64>,
        submission: bool,
    ) -> Result<(), MusicRepoError> {
        ScrobbleRepository::new(self.pool.clone()).scrobble(user_id, song_id, time, submission)?;

        if submission && let Some(song) = self.get_song(song_id)? {
            let timestamp = time.unwrap_or_else(|| chrono::Utc::now().timestamp());
            self.submit_lastfm_scrobble(user_id, &song, timestamp);
        }

        Ok(())
    }

    pub(in crate::api) fn set_now_playing(
        &self,
        user_id: i32,
        song_id: i32,
        player_id: Option<&str>,
    ) -> Result<(), MusicRepoError> {
        NowPlayingRepository::new(self.pool.clone())
            .set_now_playing(user_id, song_id, player_id)?;

        if let Some(song) = self.get_song(song_id)? {
            self.update_lastfm_now_playing(user_id, &song);
        }

        Ok(())
    }

    pub(in crate::api) fn get_now_playing(&self) -> Result<Vec<NowPlayingEntry>, MusicRepoError> {
        NowPlayingRepository::new(self.pool.clone()).get_all_now_playing()
    }

    fn submit_lastfm_scrobble(&self, user_id: i32, song: &Song, timestamp: i64) {
        let Ok(Some(session_key)) =
            UserRepository::new(self.pool.clone()).get_lastfm_session_key(user_id)
        else {
            return;
        };

        let client = self.lastfm_client.clone();
        let artist = song.artist_name.clone().unwrap_or_default();
        let track = song.title.clone();
        let album = song.album_name.clone();

        tokio::spawn(async move {
            if let Err(e) = client
                .scrobble(&session_key, &artist, &track, album.as_deref(), timestamp)
                .await
            {
                tracing::warn!(error = %e, "Failed to submit scrobble to Last.fm");
            } else {
                tracing::debug!(artist = %artist, track = %track, "Submitted scrobble to Last.fm");
            }
        });
    }

    fn update_lastfm_now_playing(&self, user_id: i32, song: &Song) {
        let Ok(Some(session_key)) =
            UserRepository::new(self.pool.clone()).get_lastfm_session_key(user_id)
        else {
            return;
        };

        let client = self.lastfm_client.clone();
        let artist = song.artist_name.clone().unwrap_or_default();
        let track = song.title.clone();
        let album = song.album_name.clone();
        let duration = Some(song.duration);

        tokio::spawn(async move {
            if let Err(e) = client
                .update_now_playing(&session_key, &artist, &track, album.as_deref(), duration)
                .await
            {
                tracing::debug!(error = %e, "Failed to update Last.fm now playing");
            } else {
                tracing::debug!(artist = %artist, track = %track, "Updated Last.fm now playing");
            }
        });
    }

    // ========================================================================
    // Ratings
    // ========================================================================

    pub(in crate::api) fn set_song_rating(
        &self,
        user_id: i32,
        song_id: i32,
        rating: i32,
    ) -> Result<(), MusicRepoError> {
        RatingRepository::new(self.pool.clone()).set_song_rating(user_id, song_id, rating)
    }

    // ========================================================================
    // Playlists
    // ========================================================================

    pub(in crate::api) fn get_playlists(
        &self,
        user_id: i32,
        username: &str,
    ) -> Result<Vec<crate::db::Playlist>, MusicRepoError> {
        PlaylistRepository::new(self.pool.clone()).get_playlists(user_id, username)
    }

    pub(in crate::api) fn get_playlist(
        &self,
        playlist_id: i32,
    ) -> Result<Option<crate::db::Playlist>, MusicRepoError> {
        PlaylistRepository::new(self.pool.clone()).get_playlist(playlist_id)
    }

    pub(in crate::api) fn get_playlist_songs(
        &self,
        playlist_id: i32,
    ) -> Result<Vec<Song>, MusicRepoError> {
        PlaylistRepository::new(self.pool.clone()).get_playlist_songs(playlist_id)
    }

    pub(in crate::api) fn get_playlist_cover_arts_batch(
        &self,
        playlist_ids: &[i32],
    ) -> Result<HashMap<i32, String>, MusicRepoError> {
        PlaylistRepository::new(self.pool.clone()).get_playlist_cover_arts_batch(playlist_ids)
    }

    pub(in crate::api) fn create_playlist(
        &self,
        user_id: i32,
        name: &str,
        comment: Option<&str>,
        song_ids: &[i32],
    ) -> Result<crate::db::Playlist, MusicRepoError> {
        PlaylistRepository::new(self.pool.clone()).create_playlist(user_id, name, comment, song_ids)
    }

    pub(in crate::api) fn update_playlist(
        &self,
        playlist_id: i32,
        name: Option<&str>,
        comment: Option<&str>,
        public: Option<bool>,
        song_ids_to_add: &[i32],
        song_indices_to_remove: &[i32],
    ) -> Result<(), MusicRepoError> {
        PlaylistRepository::new(self.pool.clone()).update_playlist(
            playlist_id,
            name,
            comment,
            public,
            song_ids_to_add,
            song_indices_to_remove,
        )
    }

    pub(in crate::api) fn delete_playlist(&self, playlist_id: i32) -> Result<bool, MusicRepoError> {
        PlaylistRepository::new(self.pool.clone()).delete_playlist(playlist_id)
    }

    pub(in crate::api) fn is_playlist_owner(
        &self,
        user_id: i32,
        playlist_id: i32,
    ) -> Result<bool, MusicRepoError> {
        PlaylistRepository::new(self.pool.clone()).is_owner(user_id, playlist_id)
    }

    // ========================================================================
    // Play queue
    // ========================================================================

    pub(in crate::api) fn get_play_queue(
        &self,
        user_id: i32,
        username: &str,
    ) -> Result<Option<PlayQueue>, MusicRepoError> {
        PlayQueueRepository::new(self.pool.clone()).get_play_queue(user_id, username)
    }

    pub(in crate::api) fn save_play_queue(
        &self,
        user_id: i32,
        song_ids: &[i32],
        current_song_id: Option<i32>,
        position: Option<i64>,
        changed_by: Option<&str>,
    ) -> Result<(), MusicRepoError> {
        PlayQueueRepository::new(self.pool.clone()).save_play_queue(
            user_id,
            song_ids,
            current_song_id,
            position,
            changed_by,
        )
    }

    // ========================================================================
    // Artist info with Last.fm cache
    // ========================================================================

    pub(in crate::api) fn get_artist_info_with_cache(
        &self,
        artist_id: i32,
    ) -> Result<ArtistInfo2Response, MusicRepoError> {
        let Some(artist) = self.get_artist(artist_id)? else {
            return Ok(ArtistInfo2Response::empty());
        };

        let mut response = ArtistInfo2Response::from_artist(&artist);

        tracing::debug!(artist_id = artist_id, artist = %artist.name, "Fetching artist info");

        match ArtistInfoCacheRepository::new(self.pool.clone()).get_valid_cache(artist_id) {
            Ok(Some(cache)) => {
                tracing::debug!(artist_id = artist_id, "Using cached Last.fm info");
                response.biography = cache.biography;
                response.last_fm_url = cache.last_fm_url;
                response.small_image_url = cache.small_image_url;
                response.medium_image_url = cache.medium_image_url;
                response.large_image_url = cache.large_image_url;

                for similar_name in &cache.similar_artists {
                    if let Some(similar_artist) =
                        ArtistRepository::new(self.pool.clone()).find_by_name(similar_name)?
                    {
                        let album_count = self.get_artist_album_count(similar_artist.id)?;
                        response
                            .similar_artists
                            .push(ArtistID3Response::from_artist(
                                &similar_artist,
                                Some(saturating_i64_to_i32(album_count)),
                            ));
                    }
                }
            }
            Ok(None) => {
                let service = self.clone();
                let artist_name = artist.name;
                tokio::spawn(async move {
                    service
                        .fetch_and_cache_artist_info(artist_id, artist_name)
                        .await;
                });
            }
            Err(e) => {
                tracing::warn!(error = %e, artist_id = artist_id, "Failed to read artist cache");
            }
        }

        Ok(response)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Coordinates async Last.fm API call, image scraping, caching, and cover art download"
    )]
    async fn fetch_and_cache_artist_info(&self, artist_id: i32, artist_name: String) {
        use crate::lastfm::models::LastFmArtistCache;

        let client = self.lastfm_client.clone();
        let pool = self.pool.clone();

        match client.get_artist_info(&artist_name).await {
            Ok(Some(lastfm_artist)) => {
                let (mut small, mut medium, mut large) = extract_image_urls(&lastfm_artist.image);

                if let Some(ref page_url) = lastfm_artist.url {
                    tracing::debug!(
                        artist = %artist_name,
                        url = %page_url,
                        "Attempting to scrape artist image from page"
                    );
                    match client.fetch_artist_image_from_page(page_url).await {
                        Ok(Some(scraped_url)) => {
                            tracing::debug!(
                                artist = %artist_name,
                                url = %scraped_url,
                                "Successfully scraped artist image"
                            );
                            large = Some(scraped_url);
                            if small.is_none() {
                                small = large.clone();
                            }
                            if medium.is_none() {
                                medium = large.clone();
                            }
                        }
                        Ok(None) => {
                            tracing::debug!(
                                artist = %artist_name,
                                "No image found on scraped page"
                            );
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                artist = %artist_name,
                                "Failed to scrape artist page"
                            );
                        }
                    }
                }

                tracing::debug!(
                    artist = %artist_name,
                    small = ?small,
                    medium = ?medium,
                    large = ?large,
                    "Final Last.fm image URLs"
                );
                let bio = extract_biography(&lastfm_artist.bio);

                let similar_names: Vec<String> = lastfm_artist
                    .similar
                    .artist
                    .iter()
                    .map(|a| a.name.clone())
                    .collect();

                if large.as_deref().is_some_and(is_lastfm_placeholder_image) {
                    tracing::warn!(
                        artist = %artist_name,
                        url = ?large,
                        "Discarding Last.fm placeholder image"
                    );
                    small = small.filter(|url| !is_lastfm_placeholder_image(url));
                    medium = medium.filter(|url| !is_lastfm_placeholder_image(url));
                    large = None;
                }

                let cache = LastFmArtistCache {
                    artist_id,
                    biography: bio,
                    last_fm_url: lastfm_artist.url,
                    small_image_url: small,
                    medium_image_url: medium,
                    large_image_url: large.clone(),
                    similar_artists: similar_names,
                    updated_at: chrono::Local::now().naive_local(),
                };

                if let Err(e) = ArtistInfoCacheRepository::new(pool.clone()).save_cache(&cache) {
                    tracing::warn!(error = %e, "Failed to save artist cache");
                } else {
                    tracing::debug!(artist = %artist_name, "Cached Last.fm artist info");
                }

                if let Some(image_url) = large {
                    match download_artist_image(artist_id, &image_url).await {
                        Ok(cover_art_id) => {
                            tracing::debug!(artist = %artist_name, "Downloaded artist image");
                            if let Err(e) = ArtistRepository::new(pool.clone())
                                .update_cover_art(artist_id, Some(&cover_art_id))
                            {
                                tracing::warn!(error = %e, "Failed to update artist cover art");
                            }
                        }
                        Err(error) => {
                            tracing::warn!(
                                error = %error,
                                artist = %artist_name,
                                url = %image_url,
                                "Failed to store artist image"
                            );
                        }
                    }
                }
            }
            Ok(None) => {
                tracing::debug!(artist = %artist_name, "No Last.fm info found");
            }
            Err(e) => {
                tracing::warn!(error = %e, artist = %artist_name, "Failed to fetch Last.fm artist info");
            }
        }
    }

    pub(in crate::api) fn get_artist_info_non_id3_with_cache(
        &self,
        artist_id: i32,
    ) -> Result<crate::models::music::ArtistInfoResponse, MusicRepoError> {
        use crate::models::music::{ArtistInfoResponse, ArtistResponse};

        let info2 = self.get_artist_info_with_cache(artist_id)?;

        let similar_artists = info2
            .similar_artists
            .into_iter()
            .map(|a| ArtistResponse {
                id: a.id,
                name: a.name,
                artist_image_url: a.artist_image_url,
                starred: a.starred,
                user_rating: None,
                average_rating: None,
            })
            .collect();

        Ok(ArtistInfoResponse {
            biography: info2.biography,
            musicbrainz_id: info2.musicbrainz_id,
            last_fm_url: info2.last_fm_url,
            small_image_url: info2.small_image_url,
            medium_image_url: info2.medium_image_url,
            large_image_url: info2.large_image_url,
            similar_artists,
        })
    }
}

/// User accounts.
#[derive(Clone, Debug)]
pub struct Users {
    pool: DbPool,
}

impl Users {
    /// Create a user account handle.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub(in crate::api) fn find_user(&self, username: &str) -> Result<Option<User>, UserRepoError> {
        UserRepository::new(self.pool.clone()).find_by_username(username)
    }

    pub(in crate::api) fn find_user_by_api_key(
        &self,
        api_key: &str,
    ) -> Result<Option<User>, UserRepoError> {
        UserRepository::new(self.pool.clone()).find_by_api_key(api_key)
    }

    /// List all users.
    pub fn get_all_users(&self) -> Result<Vec<User>, UserRepoError> {
        UserRepository::new(self.pool.clone()).find_all()
    }

    /// Delete a user by name.
    pub fn delete_user(&self, username: &str) -> Result<bool, UserRepoError> {
        let user = UserRepository::new(self.pool.clone())
            .find_by_username(username)?
            .ok_or_else(|| {
                UserRepoError::new(
                    UserRepoErrorKind::NotFound,
                    format!("user '{username}' not found"),
                )
            })?;
        UserRepository::new(self.pool.clone()).delete(user.id)
    }

    pub(in crate::api) fn change_password(
        &self,
        username: &str,
        new_password: &str,
    ) -> Result<(), UserRepoError> {
        let user = UserRepository::new(self.pool.clone())
            .find_by_username(username)?
            .ok_or_else(|| {
                UserRepoError::new(
                    UserRepoErrorKind::NotFound,
                    format!("user '{username}' not found"),
                )
            })?;

        let password_hash = hash_password(new_password)
            .map_err(|error| UserRepoError::new(UserRepoErrorKind::Database, error.to_string()))?;

        if !UserRepository::new(self.pool.clone()).update_password(user.id, &password_hash)? {
            return Err(UserRepoError::new(
                UserRepoErrorKind::NotFound,
                format!("user '{username}' not found"),
            ));
        }
        if !UserRepository::new(self.pool.clone())
            .update_subsonic_password(user.id, new_password)?
        {
            return Err(UserRepoError::new(
                UserRepoErrorKind::NotFound,
                format!("user '{username}' not found"),
            ));
        }

        Ok(())
    }

    pub(in crate::api) fn create_user(
        &self,
        username: &str,
        password: &str,
        email: &str,
        roles: &UserRoles,
    ) -> Result<User, UserRepoError> {
        let password_hash = hash_password(password)
            .map_err(|error| UserRepoError::new(UserRepoErrorKind::Database, error.to_string()))?;

        let new_user = NewUser::builder(username, &password_hash)
            .subsonic_password(password)
            .email(email)
            .admin_role(roles.admin_role)
            .settings_role(roles.settings_role)
            .stream_role(roles.stream_role)
            .jukebox_role(roles.jukebox_role)
            .download_role(roles.download_role)
            .upload_role(roles.upload_role)
            .playlist_role(roles.playlist_role)
            .cover_art_role(roles.cover_art_role)
            .comment_role(roles.comment_role)
            .podcast_role(roles.podcast_role)
            .share_role(roles.share_role)
            .video_conversion_role(roles.video_conversion_role)
            .max_bit_rate(0)
            .build();

        UserRepository::new(self.pool.clone()).create(&new_user)
    }

    pub(in crate::api) fn update_user(&self, update: &UserUpdate) -> Result<bool, UserRepoError> {
        UserRepository::new(self.pool.clone()).update_user(update)
    }
}

/// Remote control sessions.
#[derive(Clone, Debug)]
pub struct RemoteSessions {
    pool: DbPool,
}

impl RemoteSessions {
    /// Create a remote session handle.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub(in crate::api) fn create_remote_session(
        &self,
        user_id: i32,
        host_device_id: &str,
        host_device_name: Option<&str>,
        ttl_seconds: i64,
    ) -> Result<RemoteSession, MusicRepoError> {
        RemoteControlRepository::new(self.pool.clone()).create_session(
            user_id,
            host_device_id,
            host_device_name,
            ttl_seconds,
        )
    }

    pub(in crate::api) fn join_remote_session(
        &self,
        user_id: i32,
        pairing_code: &str,
        controller_device_id: &str,
        controller_device_name: Option<&str>,
    ) -> Result<Option<RemoteSession>, MusicRepoError> {
        RemoteControlRepository::new(self.pool.clone()).join_session(
            pairing_code,
            user_id,
            controller_device_id,
            controller_device_name,
        )
    }

    pub(in crate::api) fn close_remote_session(
        &self,
        user_id: i32,
        session_id: &str,
    ) -> Result<bool, MusicRepoError> {
        RemoteControlRepository::new(self.pool.clone()).close_session(session_id, user_id)
    }

    pub(in crate::api) fn get_remote_session(
        &self,
        user_id: i32,
        session_id: &str,
    ) -> Result<Option<RemoteSession>, MusicRepoError> {
        RemoteControlRepository::new(self.pool.clone()).get_session_for_user(session_id, user_id)
    }

    pub(in crate::api) fn send_remote_command(
        &self,
        user_id: i32,
        session_id: &str,
        source_device_id: &str,
        command: &str,
        payload: Option<&str>,
    ) -> Result<i64, MusicRepoError> {
        RemoteControlRepository::new(self.pool.clone())
            .get_session_for_user(session_id, user_id)?
            .ok_or_else(|| {
                MusicRepoError::new(MusicRepoErrorKind::NotFound, "remote session not found")
            })?;

        RemoteControlRepository::new(self.pool.clone()).enqueue_command(
            session_id,
            source_device_id,
            command,
            payload,
        )
    }

    pub(in crate::api) fn get_remote_commands(
        &self,
        user_id: i32,
        session_id: &str,
        since_id: i64,
        limit: i64,
        exclude_device_id: &str,
    ) -> Result<Vec<RemoteCommand>, MusicRepoError> {
        RemoteControlRepository::new(self.pool.clone())
            .get_session_for_user(session_id, user_id)?
            .ok_or_else(|| {
                MusicRepoError::new(MusicRepoErrorKind::NotFound, "remote session not found")
            })?;

        RemoteControlRepository::new(self.pool.clone()).get_commands(
            session_id,
            since_id,
            limit,
            exclude_device_id,
        )
    }

    pub(in crate::api) fn update_remote_state(
        &self,
        user_id: i32,
        session_id: &str,
        updated_by_device_id: &str,
        state_json: &str,
    ) -> Result<(), MusicRepoError> {
        RemoteControlRepository::new(self.pool.clone())
            .get_session_for_user(session_id, user_id)?
            .ok_or_else(|| {
                MusicRepoError::new(MusicRepoErrorKind::NotFound, "remote session not found")
            })?;

        RemoteControlRepository::new(self.pool.clone()).update_state(
            session_id,
            updated_by_device_id,
            state_json,
        )
    }

    pub(in crate::api) fn get_remote_state(
        &self,
        user_id: i32,
        session_id: &str,
    ) -> Result<Option<RemoteState>, MusicRepoError> {
        RemoteControlRepository::new(self.pool.clone())
            .get_session_for_user(session_id, user_id)?
            .ok_or_else(|| {
                MusicRepoError::new(MusicRepoErrorKind::NotFound, "remote session not found")
            })?;

        RemoteControlRepository::new(self.pool.clone()).get_state(session_id)
    }
}
