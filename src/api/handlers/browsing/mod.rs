//! Browsing-related API handlers.

use serde::Deserialize;

pub mod directory;
pub mod indexes;
pub mod info;
pub mod lists;
pub mod retrieval;
pub mod search;

// Re-export common types
pub use directory::get_music_directory;
pub use indexes::{get_artists, get_indexes, get_music_folders};
pub use info::{
    get_album_info, get_album_info2, get_artist_info, get_artist_info2, get_lyrics,
    get_lyrics_by_song_id,
};
pub use lists::{
    get_album_list, get_album_list2, get_genres, get_random_songs, get_similar_songs,
    get_similar_songs2, get_songs_by_genre, get_starred, get_top_songs,
};
pub use retrieval::{get_album, get_artist, get_song};
pub use search::{search, search2, search3};

/// Query parameters for endpoints that require an ID.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct IdParams {
    /// The ID of the item to retrieve.
    pub id: Option<String>,
}
