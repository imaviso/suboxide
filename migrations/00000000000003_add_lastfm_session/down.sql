-- Remove Last.fm session key column from users table
DROP INDEX IF EXISTS idx_artist_lastfm_updated;
DROP TABLE IF EXISTS artist_lastfm_info;
DROP INDEX IF EXISTS idx_users_lastfm_session;
ALTER TABLE users DROP COLUMN lastfm_session_key;
