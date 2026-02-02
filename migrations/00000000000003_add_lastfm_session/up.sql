-- Add Last.fm session key column to users table
ALTER TABLE users ADD COLUMN lastfm_session_key TEXT;

-- Add index for looking up users by Last.fm session
CREATE INDEX idx_users_lastfm_session ON users(lastfm_session_key) WHERE lastfm_session_key IS NOT NULL;

-- Create table for cached Last.fm artist info
CREATE TABLE artist_lastfm_info (
    artist_id INTEGER PRIMARY KEY NOT NULL,
    biography TEXT,
    last_fm_url TEXT,
    small_image_url TEXT,
    medium_image_url TEXT,
    large_image_url TEXT,
    similar_artists TEXT, -- JSON array of similar artist names
    updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (artist_id) REFERENCES artists(id) ON DELETE CASCADE
);

-- Index for checking cache freshness
CREATE INDEX idx_artist_lastfm_updated ON artist_lastfm_info(updated_at);
