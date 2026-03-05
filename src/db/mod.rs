//! Database module for `SQLite` persistence.

pub mod connection;
pub mod repo;
pub(crate) mod schema;

#[doc(inline)]
pub use connection::{DbConfig, DbConn, DbPool, run_migrations};
#[doc(inline)]
pub use repo::{
    AlbumRepository, ArtistRepository, MusicFolderRepository, MusicRepoError, NewUser,
    NowPlayingEntry, NowPlayingRepository, PlayQueue, PlayQueueRepository, Playlist,
    PlaylistRepository, RatingRepository, RemoteCommand, RemoteControlRepository, RemoteSession,
    RemoteState, ScrobbleRepository, SongRepository, StarredRepository, UserRepoError,
    UserRepository, UserUpdate,
};
