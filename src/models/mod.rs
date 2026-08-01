//! Models for the Subsonic API.

pub mod music;
pub mod user;

// Explicit re-exports from music module
#[doc(inline)]
pub use music::{
    Album, AlbumAnnotations, AlbumID3Response, AlbumInfoResponse, AlbumList2Response,
    AlbumListResponse, AlbumWithSongsID3Response, Artist, ArtistID3Response, ArtistInfo2Response,
    ArtistInfoResponse, ArtistResponse, ArtistWithAlbumsID3Response, ArtistsID3Response,
    BookmarkResponse, BookmarksResponse, ChildResponse, DirectoryResponse, GenreResponse,
    GenresResponse, IndexID3Response, IndexResponse, IndexesResponse, InternetRadioStationResponse,
    InternetRadioStationsResponse, LyricLine, LyricsListResponse, LyricsResponse, MusicFolder,
    MusicFolderResponse, NewAlbum, NewArtist, NewMusicFolder, NewSong, NowPlayingEntryResponse,
    NowPlayingResponse, PlayQueueByIndexResponse, PlayQueueResponse, PlaylistResponse,
    PlaylistWithSongsResponse, PlaylistsResponse, RandomSongsResponse, RemoteCommandResponse,
    RemoteCommandsResponse, RemoteSessionResponse, RemoteStateResponse, SearchMatch,
    SearchResult2Response, SearchResult3Response, SearchResultResponse, SimilarSongs2Response,
    SimilarSongsResponse, Song, SongAnnotations, SongsByGenreResponse, Starred2Response,
    StarredResponse, StructuredLyrics, TokenInfoResponse, TopSongsResponse,
};

// Explicit re-exports from user module
#[doc(inline)]
pub use user::{User, UserResponse, UserRoles, UsersResponse};
