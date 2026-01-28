//! Database connection pool and management.

use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, CustomizeConnection, Pool, PooledConnection};
use diesel::sqlite::SqliteConnection;
use std::time::Duration;

/// Type alias for our connection pool.
pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

/// Type alias for a pooled connection.
pub type DbConn = PooledConnection<ConnectionManager<SqliteConnection>>;

/// Database configuration.
#[derive(Debug, Clone)]
pub struct DbConfig {
    /// Path to the `SQLite` database file.
    pub database_url: String,
    /// Maximum number of connections in the pool.
    pub max_connections: u32,
    /// Connection timeout in seconds.
    pub connection_timeout: u64,
}

impl Default for DbConfig {
    fn default() -> Self {
        Self {
            database_url: "subsonic.db".to_string(),
            max_connections: 10,
            connection_timeout: 30,
        }
    }
}

impl DbConfig {
    /// Create a new database configuration.
    pub fn new(database_url: impl Into<String>) -> Self {
        Self {
            database_url: database_url.into(),
            ..Default::default()
        }
    }

    /// Build a connection pool from this configuration.
    pub fn build_pool(&self) -> Result<DbPool, Box<dyn std::error::Error>> {
        let manager = ConnectionManager::<SqliteConnection>::new(&self.database_url);

        Pool::builder()
            .max_size(self.max_connections)
            .connection_timeout(Duration::from_secs(self.connection_timeout))
            .connection_customizer(Box::new(SqliteConnectionCustomizer))
            .build(manager)
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
    }
}

/// Customizer that applies `SQLite` PRAGMAs to each new connection.
/// This ensures all pooled connections have consistent settings.
#[derive(Debug)]
struct SqliteConnectionCustomizer;

impl CustomizeConnection<SqliteConnection, diesel::r2d2::Error> for SqliteConnectionCustomizer {
    fn on_acquire(&self, conn: &mut SqliteConnection) -> Result<(), diesel::r2d2::Error> {
        // Set busy timeout to 5 seconds - SQLite will retry on lock contention
        // instead of immediately returning SQLITE_BUSY
        diesel::sql_query("PRAGMA busy_timeout = 5000")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;

        // Enable foreign keys (not inherited across connections)
        diesel::sql_query("PRAGMA foreign_keys = ON")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;

        // Set synchronous to NORMAL for better write performance while still being safe
        diesel::sql_query("PRAGMA synchronous = NORMAL")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;

        // Increase cache size for better read performance (negative = KB, so -64000 = 64MB)
        diesel::sql_query("PRAGMA cache_size = -64000")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;

        // Enable memory-mapped I/O for faster reads (256MB)
        diesel::sql_query("PRAGMA mmap_size = 268435456")
            .execute(conn)
            .map_err(diesel::r2d2::Error::QueryError)?;

        Ok(())
    }
}

/// Run the SQL migrations to set up the database schema.
#[allow(clippy::too_many_lines)]
pub fn run_migrations(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
    // Enable WAL mode for better concurrent read/write performance
    // WAL mode allows readers to not block writers and vice versa,
    // which is important when scanning while serving API requests.
    // Note: WAL mode is persistent and only needs to be set once per database file.
    diesel::sql_query("PRAGMA journal_mode = WAL").execute(conn)?;

    // Other PRAGMAs (busy_timeout, synchronous, cache_size, foreign_keys, mmap_size)
    // are set per-connection via SqliteConnectionCustomizer in build_pool().

    // Create users table
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS users (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            username TEXT NOT NULL UNIQUE,
            password_hash TEXT NOT NULL,
            email TEXT,
            admin_role BOOLEAN NOT NULL DEFAULT FALSE,
            settings_role BOOLEAN NOT NULL DEFAULT TRUE,
            stream_role BOOLEAN NOT NULL DEFAULT TRUE,
            jukebox_role BOOLEAN NOT NULL DEFAULT FALSE,
            download_role BOOLEAN NOT NULL DEFAULT TRUE,
            upload_role BOOLEAN NOT NULL DEFAULT FALSE,
            playlist_role BOOLEAN NOT NULL DEFAULT TRUE,
            cover_art_role BOOLEAN NOT NULL DEFAULT TRUE,
            comment_role BOOLEAN NOT NULL DEFAULT FALSE,
            podcast_role BOOLEAN NOT NULL DEFAULT FALSE,
            share_role BOOLEAN NOT NULL DEFAULT FALSE,
            video_conversion_role BOOLEAN NOT NULL DEFAULT FALSE,
            max_bit_rate INTEGER NOT NULL DEFAULT 0,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            subsonic_password TEXT,
            api_key TEXT
        )
        ",
    )
    .execute(conn)?;

    // Create index for username lookups
    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_users_username ON users(username)")
        .execute(conn)?;

    // Create unique index for API key lookups (only for non-null values)
    diesel::sql_query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_users_api_key ON users(api_key) WHERE api_key IS NOT NULL"
    )
    .execute(conn)?;

    // Migration: Add api_key column if it doesn't exist (for existing databases)
    // SQLite doesn't have a simple "ADD COLUMN IF NOT EXISTS" so we check first
    let has_api_key: Result<i32, _> = diesel::sql_query(
        "SELECT COUNT(*) as cnt FROM pragma_table_info('users') WHERE name = 'api_key'",
    )
    .get_result::<CountResult>(conn)
    .map(|r| r.cnt);

    if has_api_key.unwrap_or(0) == 0 {
        // Column doesn't exist, try to add it
        let _ = diesel::sql_query("ALTER TABLE users ADD COLUMN api_key TEXT").execute(conn);
    }

    // Create music_folders table
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS music_folders (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            name TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            enabled BOOLEAN NOT NULL DEFAULT TRUE,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        ",
    )
    .execute(conn)?;

    // Create artists table
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS artists (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            name TEXT NOT NULL,
            sort_name TEXT,
            musicbrainz_id TEXT,
            cover_art TEXT,
            artist_image_url TEXT,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        ",
    )
    .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_artists_name ON artists(name)")
        .execute(conn)?;

    // Create albums table
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS albums (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            name TEXT NOT NULL,
            sort_name TEXT,
            artist_id INTEGER REFERENCES artists(id),
            artist_name TEXT,
            year INTEGER,
            genre TEXT,
            cover_art TEXT,
            musicbrainz_id TEXT,
            duration INTEGER NOT NULL DEFAULT 0,
            song_count INTEGER NOT NULL DEFAULT 0,
            play_count INTEGER NOT NULL DEFAULT 0,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        ",
    )
    .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_albums_name ON albums(name)")
        .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_albums_artist_id ON albums(artist_id)")
        .execute(conn)?;

    // Create songs table
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS songs (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            title TEXT NOT NULL,
            sort_name TEXT,
            album_id INTEGER REFERENCES albums(id),
            artist_id INTEGER REFERENCES artists(id),
            artist_name TEXT,
            album_name TEXT,
            music_folder_id INTEGER NOT NULL REFERENCES music_folders(id),
            path TEXT NOT NULL UNIQUE,
            parent_path TEXT NOT NULL,
            file_size BIGINT NOT NULL DEFAULT 0,
            content_type TEXT NOT NULL,
            suffix TEXT NOT NULL,
            duration INTEGER NOT NULL DEFAULT 0,
            bit_rate INTEGER,
            bit_depth INTEGER,
            sampling_rate INTEGER,
            channel_count INTEGER,
            track_number INTEGER,
            disc_number INTEGER,
            year INTEGER,
            genre TEXT,
            cover_art TEXT,
            musicbrainz_id TEXT,
            play_count INTEGER NOT NULL DEFAULT 0,
            file_modified_at BIGINT,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        ",
    )
    .execute(conn)?;

    // Migration: Add file_modified_at column if it doesn't exist (for existing databases)
    let has_file_modified_at: Result<i32, _> = diesel::sql_query(
        "SELECT COUNT(*) as cnt FROM pragma_table_info('songs') WHERE name = 'file_modified_at'",
    )
    .get_result::<CountResult>(conn)
    .map(|r| r.cnt);

    if has_file_modified_at.unwrap_or(0) == 0 {
        let _ =
            diesel::sql_query("ALTER TABLE songs ADD COLUMN file_modified_at BIGINT").execute(conn);
    }

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_songs_title ON songs(title)")
        .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_songs_album_id ON songs(album_id)")
        .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_songs_artist_id ON songs(artist_id)")
        .execute(conn)?;

    diesel::sql_query(
        "CREATE INDEX IF NOT EXISTS idx_songs_music_folder_id ON songs(music_folder_id)",
    )
    .execute(conn)?;

    // Additional indexes for common query patterns
    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_songs_genre ON songs(genre)")
        .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_songs_year ON songs(year)").execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_songs_artist_name ON songs(artist_name)")
        .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_albums_genre ON albums(genre)")
        .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_albums_year ON albums(year)")
        .execute(conn)?;

    // Composite index for album queries by artist and year
    diesel::sql_query(
        "CREATE INDEX IF NOT EXISTS idx_albums_artist_year ON albums(artist_id, year)",
    )
    .execute(conn)?;

    // Create starred table for favorites
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS starred (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            artist_id INTEGER REFERENCES artists(id) ON DELETE CASCADE,
            album_id INTEGER REFERENCES albums(id) ON DELETE CASCADE,
            song_id INTEGER REFERENCES songs(id) ON DELETE CASCADE,
            starred_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            CHECK (
                (artist_id IS NOT NULL AND album_id IS NULL AND song_id IS NULL) OR
                (artist_id IS NULL AND album_id IS NOT NULL AND song_id IS NULL) OR
                (artist_id IS NULL AND album_id IS NULL AND song_id IS NOT NULL)
            )
        )
        ",
    )
    .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_starred_user_id ON starred(user_id)")
        .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_starred_artist_id ON starred(artist_id)")
        .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_starred_album_id ON starred(album_id)")
        .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_starred_song_id ON starred(song_id)")
        .execute(conn)?;

    // Unique constraint to prevent duplicate stars
    diesel::sql_query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_starred_user_artist ON starred(user_id, artist_id) WHERE artist_id IS NOT NULL"
    )
    .execute(conn)?;

    diesel::sql_query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_starred_user_album ON starred(user_id, album_id) WHERE album_id IS NOT NULL"
    )
    .execute(conn)?;

    diesel::sql_query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_starred_user_song ON starred(user_id, song_id) WHERE song_id IS NOT NULL"
    )
    .execute(conn)?;

    // Create now_playing table for currently playing songs
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS now_playing (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
            player_id TEXT,
            started_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            minutes_ago INTEGER NOT NULL DEFAULT 0
        )
        ",
    )
    .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_now_playing_user_id ON now_playing(user_id)")
        .execute(conn)?;

    // Only one "now playing" entry per user
    diesel::sql_query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_now_playing_user ON now_playing(user_id)",
    )
    .execute(conn)?;

    // Create scrobbles table for play history
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS scrobbles (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
            played_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            submission BOOLEAN NOT NULL DEFAULT TRUE
        )
        ",
    )
    .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_scrobbles_user_id ON scrobbles(user_id)")
        .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_scrobbles_song_id ON scrobbles(song_id)")
        .execute(conn)?;

    diesel::sql_query(
        "CREATE INDEX IF NOT EXISTS idx_scrobbles_played_at ON scrobbles(played_at DESC)",
    )
    .execute(conn)?;

    // Create user_ratings table for song/album/artist ratings
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS user_ratings (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            song_id INTEGER REFERENCES songs(id) ON DELETE CASCADE,
            album_id INTEGER REFERENCES albums(id) ON DELETE CASCADE,
            artist_id INTEGER REFERENCES artists(id) ON DELETE CASCADE,
            rating INTEGER NOT NULL CHECK (rating >= 0 AND rating <= 5),
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            CHECK (
                (song_id IS NOT NULL AND album_id IS NULL AND artist_id IS NULL) OR
                (song_id IS NULL AND album_id IS NOT NULL AND artist_id IS NULL) OR
                (song_id IS NULL AND album_id IS NULL AND artist_id IS NOT NULL)
            )
        )
        ",
    )
    .execute(conn)?;

    diesel::sql_query(
        "CREATE INDEX IF NOT EXISTS idx_user_ratings_user_id ON user_ratings(user_id)",
    )
    .execute(conn)?;

    diesel::sql_query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_ratings_user_song ON user_ratings(user_id, song_id) WHERE song_id IS NOT NULL"
    )
    .execute(conn)?;

    diesel::sql_query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_ratings_user_album ON user_ratings(user_id, album_id) WHERE album_id IS NOT NULL"
    )
    .execute(conn)?;

    diesel::sql_query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_ratings_user_artist ON user_ratings(user_id, artist_id) WHERE artist_id IS NOT NULL"
    )
    .execute(conn)?;

    // Create playlists table
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS playlists (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            comment TEXT,
            public BOOLEAN NOT NULL DEFAULT FALSE,
            song_count INTEGER NOT NULL DEFAULT 0,
            duration INTEGER NOT NULL DEFAULT 0,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        ",
    )
    .execute(conn)?;

    diesel::sql_query("CREATE INDEX IF NOT EXISTS idx_playlists_user_id ON playlists(user_id)")
        .execute(conn)?;

    // Create playlist_songs table
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS playlist_songs (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
            position INTEGER NOT NULL,
            created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
        ",
    )
    .execute(conn)?;

    diesel::sql_query(
        "CREATE INDEX IF NOT EXISTS idx_playlist_songs_playlist_id ON playlist_songs(playlist_id)",
    )
    .execute(conn)?;

    diesel::sql_query(
        "CREATE INDEX IF NOT EXISTS idx_playlist_songs_song_id ON playlist_songs(song_id)",
    )
    .execute(conn)?;

    // Create play_queue table for per-user play queue state
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS play_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
            current_song_id INTEGER REFERENCES songs(id) ON DELETE SET NULL,
            position BIGINT,
            changed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
            changed_by TEXT
        )
        ",
    )
    .execute(conn)?;

    diesel::sql_query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_play_queue_user_id ON play_queue(user_id)",
    )
    .execute(conn)?;

    // Create play_queue_songs table for songs in the play queue
    diesel::sql_query(
        r"
        CREATE TABLE IF NOT EXISTS play_queue_songs (
            id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
            play_queue_id INTEGER NOT NULL REFERENCES play_queue(id) ON DELETE CASCADE,
            song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
            position INTEGER NOT NULL
        )
        ",
    )
    .execute(conn)?;

    diesel::sql_query(
        "CREATE INDEX IF NOT EXISTS idx_play_queue_songs_queue_id ON play_queue_songs(play_queue_id)"
    )
    .execute(conn)?;

    Ok(())
}

/// Helper struct for count queries
#[derive(QueryableByName)]
struct CountResult {
    #[diesel(sql_type = diesel::sql_types::Integer)]
    cnt: i32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = DbConfig::default();
        assert_eq!(config.database_url, "subsonic.db");
        assert_eq!(config.max_connections, 10);
    }

    #[test]
    fn test_in_memory_pool() {
        let config = DbConfig::new(":memory:");
        let pool = config.build_pool();
        assert!(pool.is_ok());
    }
}
