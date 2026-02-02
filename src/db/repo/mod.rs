//! Database repository module.

pub mod error;
pub mod interaction;
pub mod music;
pub mod playlist;
pub mod user;

// Re-export error types
pub use error::{MusicRepoError, MusicRepoErrorKind, UserRepoError, UserRepoErrorKind};

// Re-export interaction types
pub use interaction::{
    NowPlayingEntry, NowPlayingRepository, RatingRepository, ScrobbleRepository, StarredRepository,
};

// Re-export music types
pub use music::{AlbumRepository, ArtistRepository, MusicFolderRepository, SongRepository};

// Re-export playlist types
pub use playlist::{PlayQueue, PlayQueueRepository, Playlist, PlaylistRepository};

// Re-export user types
pub use user::{NewUser, UserRepository, UserUpdate};
