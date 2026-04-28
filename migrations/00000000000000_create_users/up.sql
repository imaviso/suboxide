CREATE TABLE users (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    email TEXT,
    
    -- Roles/permissions
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
    
    -- Settings
    max_bit_rate INTEGER NOT NULL DEFAULT 0,
    
    -- Timestamps
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    subsonic_password TEXT
);

CREATE INDEX idx_users_username ON users(username);

CREATE TABLE music_folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name TEXT NOT NULL,
    path TEXT NOT NULL UNIQUE,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE artists (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    name TEXT NOT NULL,
    sort_name TEXT,
    musicbrainz_id TEXT,
    cover_art TEXT,
    artist_image_url TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_artists_name ON artists(name);

CREATE TABLE albums (
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
);

CREATE INDEX idx_albums_name ON albums(name);
CREATE INDEX idx_albums_artist_id ON albums(artist_id);

CREATE TABLE songs (
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
    file_size BIGINT NOT NULL,
    content_type TEXT NOT NULL,
    suffix TEXT NOT NULL,
    duration INTEGER NOT NULL,
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
);

CREATE INDEX idx_songs_album_id ON songs(album_id);
CREATE INDEX idx_songs_artist_id ON songs(artist_id);
CREATE INDEX idx_songs_music_folder_id ON songs(music_folder_id);
CREATE INDEX idx_songs_path ON songs(path);
CREATE INDEX idx_songs_title ON songs(title);

CREATE TABLE now_playing (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    player_id TEXT,
    started_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    minutes_ago INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX idx_now_playing_user ON now_playing(user_id);

CREATE TABLE scrobbles (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    played_at TIMESTAMP NOT NULL,
    submission BOOLEAN NOT NULL DEFAULT TRUE
);

CREATE TABLE user_ratings (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    song_id INTEGER REFERENCES songs(id) ON DELETE CASCADE,
    album_id INTEGER REFERENCES albums(id) ON DELETE CASCADE,
    artist_id INTEGER REFERENCES artists(id) ON DELETE CASCADE,
    rating INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CHECK (rating BETWEEN 1 AND 5),
    CHECK (
        (song_id IS NOT NULL AND album_id IS NULL AND artist_id IS NULL) OR
        (song_id IS NULL AND album_id IS NOT NULL AND artist_id IS NULL) OR
        (song_id IS NULL AND album_id IS NULL AND artist_id IS NOT NULL)
    )
);

CREATE UNIQUE INDEX idx_user_ratings_song ON user_ratings(user_id, song_id) WHERE song_id IS NOT NULL;
CREATE UNIQUE INDEX idx_user_ratings_album ON user_ratings(user_id, album_id) WHERE album_id IS NOT NULL;
CREATE UNIQUE INDEX idx_user_ratings_artist ON user_ratings(user_id, artist_id) WHERE artist_id IS NOT NULL;

CREATE TABLE playlists (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    comment TEXT,
    public BOOLEAN NOT NULL DEFAULT FALSE,
    song_count INTEGER NOT NULL DEFAULT 0,
    duration INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE playlist_songs (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE UNIQUE INDEX idx_playlist_songs_position ON playlist_songs(playlist_id, position);

CREATE TABLE play_queue (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    user_id INTEGER NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
    current_song_id INTEGER REFERENCES songs(id) ON DELETE SET NULL,
    position BIGINT,
    changed_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    changed_by TEXT
);

CREATE TABLE play_queue_songs (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    play_queue_id INTEGER NOT NULL REFERENCES play_queue(id) ON DELETE CASCADE,
    song_id INTEGER NOT NULL REFERENCES songs(id) ON DELETE CASCADE,
    position INTEGER NOT NULL
);

CREATE TABLE remote_sessions (
    session_id TEXT PRIMARY KEY NOT NULL,
    pairing_code TEXT NOT NULL UNIQUE,
    owner_user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    host_device_id TEXT NOT NULL,
    host_device_name TEXT,
    controller_user_id INTEGER REFERENCES users(id) ON DELETE SET NULL,
    controller_device_id TEXT,
    controller_device_name TEXT,
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    closed_at TIMESTAMP
);

CREATE INDEX idx_remote_sessions_owner ON remote_sessions(owner_user_id);
CREATE INDEX idx_remote_sessions_expires ON remote_sessions(expires_at);

CREATE TABLE remote_commands (
    id INTEGER PRIMARY KEY AUTOINCREMENT NOT NULL,
    session_id TEXT NOT NULL REFERENCES remote_sessions(session_id) ON DELETE CASCADE,
    source_device_id TEXT NOT NULL,
    command TEXT NOT NULL,
    payload TEXT,
    created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX idx_remote_commands_session_id ON remote_commands(session_id);

CREATE TABLE remote_state (
    session_id TEXT PRIMARY KEY NOT NULL REFERENCES remote_sessions(session_id) ON DELETE CASCADE,
    state_json TEXT NOT NULL,
    updated_by_device_id TEXT NOT NULL,
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
);
