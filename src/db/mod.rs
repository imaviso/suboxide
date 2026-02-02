//! Database module for `SQLite` persistence.

pub mod connection;
pub mod repo;
pub(crate) mod schema;

pub use connection::{DbConfig, DbConn, DbPool, run_migrations};
pub use repo::{
    AlbumRepository, ArtistRepository, MusicFolderRepository, MusicRepoError, NewUser,
    NowPlayingEntry, NowPlayingRepository, PlayQueue, PlayQueueRepository, Playlist,
    PlaylistRepository, RatingRepository, ScrobbleRepository, SongRepository, StarredRepository,
    UserRepoError, UserRepository, UserUpdate,
};
