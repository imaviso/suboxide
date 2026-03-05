//! Subsonic API handlers.

pub mod annotation;
pub mod browsing;
pub mod media;
pub mod playlists;
pub mod playqueue;
pub mod remote;
pub mod scanning;
pub mod system;
pub mod users;

// Annotation handlers
#[doc(inline)]
pub use annotation::{get_now_playing, get_starred2, scrobble, set_rating, star, unstar};

// Browsing handlers (re-exported from browsing module)
#[doc(inline)]
pub use browsing::{
    IdParams, get_album, get_album_info, get_album_info2, get_album_list, get_album_list2,
    get_artist, get_artist_info, get_artist_info2, get_artists, get_genres, get_indexes,
    get_lyrics, get_lyrics_by_song_id, get_music_directory, get_music_folders, get_random_songs,
    get_similar_songs, get_similar_songs2, get_song, get_songs_by_genre, get_starred,
    get_top_songs, search, search2, search3,
};

// Media handlers
#[doc(inline)]
pub use media::{CoverArtParams, StreamParams, download, get_cover_art, stream};

// Playlist handlers
#[doc(inline)]
pub use playlists::{
    CreatePlaylistParams, DeletePlaylistParams, GetPlaylistParams, GetPlaylistsParams,
    UpdatePlaylistParams, create_playlist, delete_playlist, get_playlist, get_playlists,
    update_playlist,
};

// Play queue handlers
#[doc(inline)]
pub use playqueue::{
    get_play_queue, get_play_queue_by_index, save_play_queue, save_play_queue_by_index,
};

// Remote control handlers
#[doc(inline)]
pub use remote::{
    close_remote_session, create_remote_session, get_remote_commands, get_remote_session,
    get_remote_state, join_remote_session, send_remote_command, update_remote_state,
};

// Scanning handlers
#[doc(inline)]
pub use scanning::{get_scan_status, start_scan};

// System handlers
#[doc(inline)]
pub use system::{get_bookmarks, get_license, get_open_subsonic_extensions, ping, token_info};

// User handlers
#[doc(inline)]
pub use users::{
    ChangePasswordParams, CreateUserParams, DeleteUserParams, GetUserParams, UpdateUserParams,
    change_password, create_user, delete_user, get_user, get_users, update_user,
};
