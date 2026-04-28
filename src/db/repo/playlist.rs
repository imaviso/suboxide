//! Playlist and play queue repository operations.

use chrono::NaiveDateTime;
use diesel::prelude::*;

use crate::db::DbPool;
use crate::db::repo::error::{MusicRepoError, MusicRepoErrorKind};
use crate::db::repo::music::SongRow;
use crate::db::repo::user::UserRow;
use crate::db::schema::{play_queue, play_queue_songs, playlist_songs, playlists, songs, users};
use crate::models::music::Song;

fn usize_to_i32(value: usize, field: &str) -> Result<i32, MusicRepoError> {
    i32::try_from(value).map_err(|error| {
        MusicRepoError::new(
            MusicRepoErrorKind::Database,
            format!("{field} exceeds i32 range: {error}"),
        )
    })
}

// ============================================================================
// Playlist Repository
// ============================================================================

/// Database row representation for playlists.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = playlists)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PlaylistRow {
    pub id: i32,
    pub user_id: i32,
    pub name: String,
    pub comment: Option<String>,
    pub public: bool,
    pub song_count: i32,
    pub duration: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Data for inserting a new playlist.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = playlists)]
pub struct NewPlaylist<'a> {
    pub user_id: i32,
    pub name: &'a str,
    pub comment: Option<&'a str>,
    pub public: bool,
}

/// Database row representation for playlist songs.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = playlist_songs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PlaylistSongRow {
    pub id: i32,
    pub playlist_id: i32,
    pub song_id: i32,
    pub position: i32,
    pub created_at: NaiveDateTime,
}

/// Data for inserting a playlist song.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = playlist_songs)]
pub struct NewPlaylistSong {
    pub playlist_id: i32,
    pub song_id: i32,
    pub position: i32,
}

/// Playlist with owner info.
#[derive(Debug, Clone)]
pub struct Playlist {
    pub id: i32,
    pub name: String,
    pub comment: Option<String>,
    pub owner: String,
    pub public: bool,
    pub song_count: i32,
    pub duration: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Repository for playlist database operations.
#[derive(Clone, Debug)]
pub struct PlaylistRepository {
    pool: DbPool,
}

impl PlaylistRepository {
    /// Create a new playlist repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get all playlists for a user (including public playlists from others).
    pub fn get_playlists(
        &self,
        user_id: i32,
        _username: &str,
    ) -> Result<Vec<Playlist>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Get playlists owned by user or public playlists
        let results: Vec<(PlaylistRow, UserRow)> = playlists::table
            .inner_join(users::table.on(playlists::user_id.eq(users::id)))
            .filter(
                playlists::user_id
                    .eq(user_id)
                    .or(playlists::public.eq(true)),
            )
            .select((PlaylistRow::as_select(), UserRow::as_select()))
            .order(playlists::name.asc())
            .load(&mut conn)?;

        Ok(results
            .into_iter()
            .map(|(p, u)| Playlist {
                id: p.id,
                name: p.name,
                comment: p.comment,
                owner: u.username,
                public: p.public,
                song_count: p.song_count,
                duration: p.duration,
                created_at: p.created_at,
                updated_at: p.updated_at,
            })
            .collect())
    }

    /// Get a playlist by ID.
    pub fn get_playlist(&self, playlist_id: i32) -> Result<Option<Playlist>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        Self::get_playlist_with_conn(&mut conn, playlist_id)
    }

    fn get_playlist_with_conn(
        conn: &mut diesel::SqliteConnection,
        playlist_id: i32,
    ) -> Result<Option<Playlist>, MusicRepoError> {
        let result: Option<(PlaylistRow, UserRow)> = playlists::table
            .inner_join(users::table.on(playlists::user_id.eq(users::id)))
            .filter(playlists::id.eq(playlist_id))
            .select((PlaylistRow::as_select(), UserRow::as_select()))
            .first(conn)
            .optional()?;

        Ok(result.map(|(p, u)| Playlist {
            id: p.id,
            name: p.name,
            comment: p.comment,
            owner: u.username,
            public: p.public,
            song_count: p.song_count,
            duration: p.duration,
            created_at: p.created_at,
            updated_at: p.updated_at,
        }))
    }

    /// Get songs in a playlist, ordered by position.
    pub fn get_playlist_songs(&self, playlist_id: i32) -> Result<Vec<Song>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let results: Vec<(PlaylistSongRow, SongRow)> = playlist_songs::table
            .inner_join(songs::table.on(playlist_songs::song_id.eq(songs::id)))
            .filter(playlist_songs::playlist_id.eq(playlist_id))
            .select((PlaylistSongRow::as_select(), SongRow::as_select()))
            .order(playlist_songs::position.asc())
            .load(&mut conn)?;

        Ok(results.into_iter().map(|(_, s)| Song::from(s)).collect())
    }

    /// Get cover art IDs for multiple playlists in a single query.
    /// Returns a map of `playlist_id` -> `cover_art` (from the first song in each playlist).
    pub fn get_playlist_cover_arts_batch(
        &self,
        playlist_ids: &[i32],
    ) -> Result<std::collections::HashMap<i32, String>, MusicRepoError> {
        if playlist_ids.is_empty() {
            return Ok(std::collections::HashMap::new());
        }

        let mut conn = self.pool.get()?;

        // Get the first song (position 0) for each playlist and join to get cover_art
        let results: Vec<(i32, Option<String>)> = playlist_songs::table
            .inner_join(songs::table.on(playlist_songs::song_id.eq(songs::id)))
            .filter(playlist_songs::playlist_id.eq_any(playlist_ids))
            .filter(playlist_songs::position.eq(0))
            .select((playlist_songs::playlist_id, songs::cover_art))
            .load(&mut conn)?;

        Ok(results
            .into_iter()
            .filter_map(|(pid, cover)| cover.map(|c| (pid, c)))
            .collect())
    }

    /// Create a new playlist.
    pub fn create_playlist(
        &self,
        user_id: i32,
        name: &str,
        comment: Option<&str>,
        song_ids: &[i32],
    ) -> Result<Playlist, MusicRepoError> {
        let mut conn = self.pool.get()?;

        conn.transaction(|conn| {
            let new_playlist = NewPlaylist {
                user_id,
                name,
                comment,
                public: false,
            };

            diesel::insert_into(playlists::table)
                .values(&new_playlist)
                .execute(conn)?;

            let playlist_id: i32 = playlists::table
                .filter(playlists::user_id.eq(user_id))
                .filter(playlists::name.eq(name))
                .order(playlists::created_at.desc())
                .select(playlists::id)
                .first(conn)?;

            for (position, song_id) in song_ids.iter().enumerate() {
                let exists = songs::table
                    .filter(songs::id.eq(song_id))
                    .select(songs::id)
                    .first::<i32>(conn)
                    .optional()?;

                if exists.is_none() {
                    return Err(MusicRepoError::new(
                        MusicRepoErrorKind::NotFound,
                        format!("song {song_id} not found"),
                    ));
                }

                let new_song = NewPlaylistSong {
                    playlist_id,
                    song_id: *song_id,
                    position: usize_to_i32(position, "playlist song position")?,
                };

                diesel::insert_into(playlist_songs::table)
                    .values(&new_song)
                    .execute(conn)?;
            }

            Self::update_playlist_stats(conn, playlist_id)?;

            Self::get_playlist_with_conn(conn, playlist_id)?.ok_or_else(|| {
                MusicRepoError::new(MusicRepoErrorKind::NotFound, "Playlist not found")
            })
        })
    }

    /// Update a playlist (name/comment/songs).
    pub fn update_playlist(
        &self,
        playlist_id: i32,
        name: Option<&str>,
        comment: Option<&str>,
        public: Option<bool>,
        song_ids_to_add: &[i32],
        song_indices_to_remove: &[i32],
    ) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        conn.transaction(|conn| {
            if let Some(n) = name {
                diesel::update(playlists::table.filter(playlists::id.eq(playlist_id)))
                    .set(playlists::name.eq(n))
                    .execute(conn)?;
            }

            if let Some(c) = comment {
                diesel::update(playlists::table.filter(playlists::id.eq(playlist_id)))
                    .set(playlists::comment.eq(c))
                    .execute(conn)?;
            }

            if let Some(p) = public {
                diesel::update(playlists::table.filter(playlists::id.eq(playlist_id)))
                    .set(playlists::public.eq(p))
                    .execute(conn)?;
            }

            for index in song_indices_to_remove {
                diesel::delete(
                    playlist_songs::table
                        .filter(playlist_songs::playlist_id.eq(playlist_id))
                        .filter(playlist_songs::position.eq(index)),
                )
                .execute(conn)?;
            }

            if !song_indices_to_remove.is_empty() {
                Self::renumber_positions(conn, playlist_id)?;
            }

            if !song_ids_to_add.is_empty() {
                let max_pos: Option<i32> = playlist_songs::table
                    .filter(playlist_songs::playlist_id.eq(playlist_id))
                    .select(diesel::dsl::max(playlist_songs::position))
                    .first(conn)?;

                let start_pos = max_pos.unwrap_or(-1) + 1;

                for (offset, song_id) in song_ids_to_add.iter().enumerate() {
                    let next_pos = start_pos
                        .checked_add(usize_to_i32(offset, "playlist song position")?)
                        .ok_or_else(|| {
                            MusicRepoError::new(
                                MusicRepoErrorKind::Database,
                                "playlist song position exceeds i32 range",
                            )
                        })?;
                    let new_song = NewPlaylistSong {
                        playlist_id,
                        song_id: *song_id,
                        position: next_pos,
                    };

                    diesel::insert_into(playlist_songs::table)
                        .values(&new_song)
                        .execute(conn)?;
                }
            }

            Self::update_playlist_stats(conn, playlist_id)
        })
    }

    /// Delete a playlist.
    pub fn delete_playlist(&self, playlist_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Delete playlist songs first (should cascade, but be explicit)
        diesel::delete(playlist_songs::table.filter(playlist_songs::playlist_id.eq(playlist_id)))
            .execute(&mut conn)?;

        // Delete playlist
        let deleted = diesel::delete(playlists::table.filter(playlists::id.eq(playlist_id)))
            .execute(&mut conn)?;

        Ok(deleted > 0)
    }

    /// Check if user owns a playlist.
    pub fn is_owner(&self, user_id: i32, playlist_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let owner_id: Option<i32> = playlists::table
            .filter(playlists::id.eq(playlist_id))
            .select(playlists::user_id)
            .first(&mut conn)
            .optional()?;

        Ok(owner_id == Some(user_id))
    }

    /// Helper to renumber positions after removal.
    fn renumber_positions(
        conn: &mut diesel::SqliteConnection,
        playlist_id: i32,
    ) -> Result<(), MusicRepoError> {
        // Get all playlist songs ordered by current position
        let song_ids: Vec<i32> = playlist_songs::table
            .filter(playlist_songs::playlist_id.eq(playlist_id))
            .order(playlist_songs::position.asc())
            .select(playlist_songs::id)
            .load(conn)?;

        // Update positions
        for (new_pos, id) in song_ids.iter().enumerate() {
            diesel::update(playlist_songs::table.filter(playlist_songs::id.eq(id)))
                .set(playlist_songs::position.eq(usize_to_i32(new_pos, "playlist song position")?))
                .execute(conn)?;
        }

        Ok(())
    }

    /// Helper to update playlist stats (`song_count`, duration).
    fn update_playlist_stats(
        conn: &mut diesel::SqliteConnection,
        playlist_id: i32,
    ) -> Result<(), MusicRepoError> {
        // Count songs and sum duration
        let results: Vec<SongRow> = playlist_songs::table
            .inner_join(songs::table.on(playlist_songs::song_id.eq(songs::id)))
            .filter(playlist_songs::playlist_id.eq(playlist_id))
            .select(SongRow::as_select())
            .load(conn)?;

        let song_count = usize_to_i32(results.len(), "playlist song count")?;
        let total_duration: i32 = results.iter().map(|s| s.duration).sum();

        diesel::update(playlists::table.filter(playlists::id.eq(playlist_id)))
            .set((
                playlists::song_count.eq(song_count),
                playlists::duration.eq(total_duration),
                playlists::updated_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .execute(conn)?;

        Ok(())
    }
}

// ============================================================================
// Play Queue Repository
// ============================================================================

/// Database row representation for play queue.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = play_queue)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PlayQueueRow {
    pub id: i32,
    pub user_id: i32,
    pub current_song_id: Option<i32>,
    pub position: Option<i64>,
    pub changed_at: NaiveDateTime,
    pub changed_by: Option<String>,
}

/// Database row representation for play queue songs.
#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = play_queue_songs)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
pub struct PlayQueueSongRow {
    pub id: i32,
    pub play_queue_id: i32,
    pub song_id: i32,
    pub position: i32,
}

/// Data for inserting a play queue.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = play_queue)]
pub struct NewPlayQueue {
    pub user_id: i32,
    pub current_song_id: Option<i32>,
    pub position: Option<i64>,
    pub changed_by: Option<String>,
}

/// Data for inserting a play queue song.
#[derive(Debug, Clone, Insertable)]
#[diesel(table_name = play_queue_songs)]
pub struct NewPlayQueueSong {
    pub play_queue_id: i32,
    pub song_id: i32,
    pub position: i32,
}

/// Play queue with songs.
#[derive(Debug, Clone)]
pub struct PlayQueue {
    pub current_song: Option<Song>,
    pub position: Option<i64>,
    pub songs: Vec<Song>,
    pub changed_at: NaiveDateTime,
    pub changed_by: Option<String>,
    pub username: String,
}

/// Repository for play queue database operations.
#[derive(Clone, Debug)]
pub struct PlayQueueRepository {
    pool: DbPool,
}

impl PlayQueueRepository {
    /// Create a new play queue repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Get the play queue for a user.
    pub fn get_play_queue(
        &self,
        user_id: i32,
        username: &str,
    ) -> Result<Option<PlayQueue>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        // Get the play queue
        let queue: Option<PlayQueueRow> = play_queue::table
            .filter(play_queue::user_id.eq(user_id))
            .select(PlayQueueRow::as_select())
            .first(&mut conn)
            .optional()?;

        let Some(queue) = queue else { return Ok(None) };

        // Get the current song
        let current_song = if let Some(song_id) = queue.current_song_id {
            songs::table
                .filter(songs::id.eq(song_id))
                .select(SongRow::as_select())
                .first(&mut conn)
                .optional()?
                .map(Song::from)
        } else {
            None
        };

        // Get all songs in the queue
        let song_rows: Vec<SongRow> = play_queue_songs::table
            .inner_join(songs::table.on(play_queue_songs::song_id.eq(songs::id)))
            .filter(play_queue_songs::play_queue_id.eq(queue.id))
            .order(play_queue_songs::position.asc())
            .select(SongRow::as_select())
            .load(&mut conn)?;

        let queue_songs: Vec<Song> = song_rows.into_iter().map(Song::from).collect();

        Ok(Some(PlayQueue {
            current_song,
            position: queue.position,
            songs: queue_songs,
            changed_at: queue.changed_at,
            changed_by: queue.changed_by,
            username: username.to_string(),
        }))
    }

    /// Save the play queue for a user.
    pub fn save_play_queue(
        &self,
        user_id: i32,
        song_ids: &[i32],
        current_song_id: Option<i32>,
        position: Option<i64>,
        changed_by: Option<&str>,
    ) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;

        conn.transaction(|conn| {
            for song_id in song_ids {
                let exists = songs::table
                    .filter(songs::id.eq(song_id))
                    .select(songs::id)
                    .first::<i32>(conn)
                    .optional()?;
                if exists.is_none() {
                    return Err(MusicRepoError::new(
                        MusicRepoErrorKind::NotFound,
                        format!("song {song_id} not found"),
                    ));
                }
            }

            let queue_id: i32 = {
                let existing: Option<i32> = play_queue::table
                    .filter(play_queue::user_id.eq(user_id))
                    .select(play_queue::id)
                    .first(conn)
                    .optional()?;

                if let Some(id) = existing {
                    diesel::update(play_queue::table.filter(play_queue::id.eq(id)))
                        .set((
                            play_queue::current_song_id.eq(current_song_id),
                            play_queue::position.eq(position),
                            play_queue::changed_at.eq(chrono::Utc::now().naive_utc()),
                            play_queue::changed_by.eq(changed_by),
                        ))
                        .execute(conn)?;
                    id
                } else {
                    let new_queue = NewPlayQueue {
                        user_id,
                        current_song_id,
                        position,
                        changed_by: changed_by.map(std::string::ToString::to_string),
                    };

                    diesel::insert_into(play_queue::table)
                        .values(&new_queue)
                        .execute(conn)?;

                    play_queue::table
                        .filter(play_queue::user_id.eq(user_id))
                        .select(play_queue::id)
                        .first(conn)?
                }
            };

            diesel::delete(
                play_queue_songs::table.filter(play_queue_songs::play_queue_id.eq(queue_id)),
            )
            .execute(conn)?;

            for (pos, song_id) in song_ids.iter().enumerate() {
                let new_song = NewPlayQueueSong {
                    play_queue_id: queue_id,
                    song_id: *song_id,
                    position: usize_to_i32(pos, "play queue position")?,
                };

                diesel::insert_into(play_queue_songs::table)
                    .values(&new_song)
                    .execute(conn)?;
            }

            Ok(())
        })
    }
}
