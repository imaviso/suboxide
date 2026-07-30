//! Database repository module.

pub mod artist_cache;
pub mod error;
pub mod interaction;
pub mod music;
pub mod playlist;
pub mod remote;
pub mod settings;
pub mod user;

// Re-export artist cache types
#[doc(inline)]
pub use artist_cache::ArtistInfoCacheRepository;

// Re-export error types
#[doc(inline)]
pub use error::{MusicRepoError, MusicRepoErrorKind, UserRepoError, UserRepoErrorKind};

// Re-export interaction types
#[doc(inline)]
pub use interaction::{
    NowPlayingEntry, NowPlayingRepository, RatingRepository, ScrobbleRepository, StarredRepository,
};

// Re-export music types
#[doc(inline)]
pub use music::{AlbumRepository, ArtistRepository, MusicFolderRepository, SongRepository};

// Re-export playlist types
#[doc(inline)]
pub use playlist::{PlayQueue, PlayQueueRepository, Playlist, PlaylistRepository};

// Re-export remote control types
#[doc(inline)]
pub use remote::{RemoteCommand, RemoteControlRepository, RemoteSession, RemoteState};

// Re-export settings types
#[doc(inline)]
pub use settings::{SETTING_LASTFM_API_KEY, SETTING_LASTFM_API_SECRET, SettingsRepository};

// Re-export user types
#[doc(inline)]
pub use user::{NewUser, UserRepository, UserUpdate};
