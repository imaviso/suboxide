//! Music library models.

use chrono::NaiveDateTime;
use serde::Serialize;

/// A music folder (library root directory).
#[derive(Debug, Clone)]
pub struct MusicFolder {
    pub id: i32,
    pub name: String,
    pub path: String,
    pub enabled: bool,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Subsonic API music folder response format.
#[derive(Debug, Serialize, Clone)]
pub struct MusicFolderResponse {
    #[serde(rename = "@id")]
    pub id: i32,
    #[serde(rename = "@name", skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl From<&MusicFolder> for MusicFolderResponse {
    fn from(folder: &MusicFolder) -> Self {
        Self {
            id: folder.id,
            name: Some(folder.name.clone()),
        }
    }
}

/// An artist in the music library.
#[derive(Debug, Clone)]
pub struct Artist {
    pub id: i32,
    pub name: String,
    pub sort_name: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub cover_art: Option<String>,
    pub artist_image_url: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Subsonic API artist response format (for getIndexes).
#[derive(Debug, Serialize, Clone)]
pub struct ArtistResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@artistImageUrl", skip_serializing_if = "Option::is_none")]
    pub artist_image_url: Option<String>,
    #[serde(rename = "@starred", skip_serializing_if = "Option::is_none")]
    pub starred: Option<String>,
    #[serde(rename = "@userRating", skip_serializing_if = "Option::is_none")]
    pub user_rating: Option<i32>,
    #[serde(rename = "@averageRating", skip_serializing_if = "Option::is_none")]
    pub average_rating: Option<f64>,
}

impl From<&Artist> for ArtistResponse {
    fn from(artist: &Artist) -> Self {
        Self::from_artist_with_starred(artist, None)
    }
}

/// Subsonic API artist ID3 response format (for getArtists).
#[derive(Debug, Serialize, Clone)]
pub struct ArtistID3Response {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "@artistImageUrl", skip_serializing_if = "Option::is_none")]
    pub artist_image_url: Option<String>,
    #[serde(rename = "@albumCount", skip_serializing_if = "Option::is_none")]
    pub album_count: Option<i32>,
    #[serde(rename = "@starred", skip_serializing_if = "Option::is_none")]
    pub starred: Option<String>,
    #[serde(rename = "@musicBrainzId", skip_serializing_if = "Option::is_none")]
    pub musicbrainz_id: Option<String>,
    #[serde(rename = "@sortName", skip_serializing_if = "Option::is_none")]
    pub sort_name: Option<String>,
}

impl ArtistID3Response {
    #[must_use]
    pub fn from_artist(artist: &Artist, album_count: Option<i32>) -> Self {
        Self {
            id: artist.id.to_string(),
            name: artist.name.clone(),
            cover_art: artist.cover_art.clone(),
            artist_image_url: artist.artist_image_url.clone(),
            album_count,
            starred: None,
            musicbrainz_id: artist.musicbrainz_id.clone(),
            sort_name: artist.sort_name.clone(),
        }
    }

    #[must_use]
    pub fn from_artist_with_starred(
        artist: &Artist,
        album_count: Option<i32>,
        starred_at: Option<&NaiveDateTime>,
    ) -> Self {
        Self {
            id: artist.id.to_string(),
            name: artist.name.clone(),
            cover_art: artist.cover_art.clone(),
            artist_image_url: artist.artist_image_url.clone(),
            album_count,
            starred: starred_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
            musicbrainz_id: artist.musicbrainz_id.clone(),
            sort_name: artist.sort_name.clone(),
        }
    }
}

/// An album in the music library.
#[derive(Debug, Clone)]
pub struct Album {
    pub id: i32,
    pub name: String,
    pub sort_name: Option<String>,
    pub artist_id: Option<i32>,
    pub artist_name: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_art: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub duration: i32,
    pub song_count: i32,
    pub play_count: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Subsonic API album ID3 response format.
#[derive(Debug, Serialize, Clone)]
pub struct AlbumID3Response {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@artist", skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(rename = "@artistId", skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<String>,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "@songCount")]
    pub song_count: i32,
    #[serde(rename = "@duration")]
    pub duration: i32,
    #[serde(rename = "@playCount", skip_serializing_if = "Option::is_none")]
    pub play_count: Option<i32>,
    #[serde(rename = "@created")]
    pub created: String,
    #[serde(rename = "@starred", skip_serializing_if = "Option::is_none")]
    pub starred: Option<String>,
    #[serde(rename = "@year", skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(rename = "@genre", skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
}

impl From<&Album> for AlbumID3Response {
    fn from(album: &Album) -> Self {
        Self {
            id: album.id.to_string(),
            name: album.name.clone(),
            artist: album.artist_name.clone(),
            artist_id: album.artist_id.map(|id| id.to_string()),
            cover_art: album.cover_art.clone(),
            song_count: album.song_count,
            duration: album.duration,
            play_count: Some(album.play_count),
            created: album
                .created_at
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
            starred: None,
            year: album.year,
            genre: album.genre.clone(),
        }
    }
}

impl AlbumID3Response {
    #[must_use]
    pub fn from_album_with_starred(album: &Album, starred_at: Option<&NaiveDateTime>) -> Self {
        Self {
            id: album.id.to_string(),
            name: album.name.clone(),
            artist: album.artist_name.clone(),
            artist_id: album.artist_id.map(|id| id.to_string()),
            cover_art: album.cover_art.clone(),
            song_count: album.song_count,
            duration: album.duration,
            play_count: Some(album.play_count),
            created: album
                .created_at
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
            starred: starred_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
            year: album.year,
            genre: album.genre.clone(),
        }
    }
}

/// A song/track in the music library.
#[derive(Debug, Clone)]
pub struct Song {
    pub id: i32,
    pub title: String,
    pub sort_name: Option<String>,
    pub album_id: Option<i32>,
    pub artist_id: Option<i32>,
    pub artist_name: Option<String>,
    pub album_name: Option<String>,
    pub music_folder_id: i32,
    pub path: String,
    pub parent_path: String,
    pub file_size: i64,
    pub content_type: String,
    pub suffix: String,
    pub duration: i32,
    pub bit_rate: Option<i32>,
    pub bit_depth: Option<i32>,
    pub sampling_rate: Option<i32>,
    pub channel_count: Option<i32>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_art: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub play_count: i32,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// Subsonic API child (song) response format.
#[derive(Debug, Serialize, Clone)]
pub struct ChildResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@parent", skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(rename = "@isDir")]
    pub is_dir: bool,
    #[serde(rename = "@title")]
    pub title: String,
    #[serde(rename = "@album", skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(rename = "@artist", skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(rename = "@track", skip_serializing_if = "Option::is_none")]
    pub track: Option<i32>,
    #[serde(rename = "@year", skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(rename = "@genre", skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "@size", skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(rename = "@contentType", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(rename = "@suffix", skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(rename = "@duration", skip_serializing_if = "Option::is_none")]
    pub duration: Option<i32>,
    #[serde(rename = "@bitRate", skip_serializing_if = "Option::is_none")]
    pub bit_rate: Option<i32>,
    #[serde(rename = "@bitDepth", skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<i32>,
    #[serde(rename = "@samplingRate", skip_serializing_if = "Option::is_none")]
    pub sampling_rate: Option<i32>,
    #[serde(rename = "@channelCount", skip_serializing_if = "Option::is_none")]
    pub channel_count: Option<i32>,
    #[serde(rename = "@path", skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(rename = "@playCount", skip_serializing_if = "Option::is_none")]
    pub play_count: Option<i32>,
    #[serde(rename = "@discNumber", skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<i32>,
    #[serde(rename = "@created", skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(rename = "@albumId", skip_serializing_if = "Option::is_none")]
    pub album_id: Option<String>,
    #[serde(rename = "@artistId", skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<String>,
    #[serde(rename = "@type", skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(rename = "@starred", skip_serializing_if = "Option::is_none")]
    pub starred: Option<String>,
}

impl From<&Song> for ChildResponse {
    fn from(song: &Song) -> Self {
        Self {
            id: song.id.to_string(),
            parent: song.album_id.map(|id| id.to_string()),
            is_dir: false,
            title: song.title.clone(),
            album: song.album_name.clone(),
            artist: song.artist_name.clone(),
            track: song.track_number,
            year: song.year,
            genre: song.genre.clone(),
            cover_art: song.cover_art.clone(),
            size: Some(song.file_size),
            content_type: Some(song.content_type.clone()),
            suffix: Some(song.suffix.clone()),
            duration: Some(song.duration),
            bit_rate: song.bit_rate,
            bit_depth: song.bit_depth,
            sampling_rate: song.sampling_rate,
            channel_count: song.channel_count,
            path: Some(song.path.clone()),
            play_count: Some(song.play_count),
            disc_number: song.disc_number,
            created: Some(song.created_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
            album_id: song.album_id.map(|id| id.to_string()),
            artist_id: song.artist_id.map(|id| id.to_string()),
            media_type: Some("music".to_string()),
            starred: None,
        }
    }
}

impl ChildResponse {
    #[must_use]
    pub fn from_song_with_starred(song: &Song, starred_at: Option<&NaiveDateTime>) -> Self {
        Self {
            id: song.id.to_string(),
            parent: song.album_id.map(|id| id.to_string()),
            is_dir: false,
            title: song.title.clone(),
            album: song.album_name.clone(),
            artist: song.artist_name.clone(),
            track: song.track_number,
            year: song.year,
            genre: song.genre.clone(),
            cover_art: song.cover_art.clone(),
            size: Some(song.file_size),
            content_type: Some(song.content_type.clone()),
            suffix: Some(song.suffix.clone()),
            duration: Some(song.duration),
            bit_rate: song.bit_rate,
            bit_depth: song.bit_depth,
            sampling_rate: song.sampling_rate,
            channel_count: song.channel_count,
            path: Some(song.path.clone()),
            play_count: Some(song.play_count),
            disc_number: song.disc_number,
            created: Some(song.created_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
            album_id: song.album_id.map(|id| id.to_string()),
            artist_id: song.artist_id.map(|id| id.to_string()),
            media_type: Some("music".to_string()),
            starred: starred_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
        }
    }
}

/// Index entry for getIndexes response.
#[derive(Debug, Serialize, Clone)]
pub struct IndexResponse {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "artist", skip_serializing_if = "Vec::is_empty")]
    pub artists: Vec<ArtistResponse>,
}

/// Indexes response for getIndexes.
#[derive(Debug, Serialize, Clone)]
pub struct IndexesResponse {
    #[serde(rename = "@ignoredArticles")]
    pub ignored_articles: String,
    #[serde(rename = "@lastModified")]
    pub last_modified: i64,
    #[serde(rename = "index", skip_serializing_if = "Vec::is_empty")]
    pub indexes: Vec<IndexResponse>,
}

/// Index entry for getArtists response (ID3 version).
#[derive(Debug, Serialize, Clone)]
pub struct IndexID3Response {
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "artist", skip_serializing_if = "Vec::is_empty")]
    pub artists: Vec<ArtistID3Response>,
}

/// Artists response for getArtists (ID3 version).
#[derive(Debug, Serialize, Clone)]
pub struct ArtistsID3Response {
    #[serde(rename = "@ignoredArticles")]
    pub ignored_articles: String,
    #[serde(rename = "index", skip_serializing_if = "Vec::is_empty")]
    pub indexes: Vec<IndexID3Response>,
}

/// Album with songs response for getAlbum.
#[derive(Debug, Serialize, Clone)]
pub struct AlbumWithSongsID3Response {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@artist", skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(rename = "@artistId", skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<String>,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "@songCount")]
    pub song_count: i32,
    #[serde(rename = "@duration")]
    pub duration: i32,
    #[serde(rename = "@playCount", skip_serializing_if = "Option::is_none")]
    pub play_count: Option<i32>,
    #[serde(rename = "@created")]
    pub created: String,
    #[serde(rename = "@starred", skip_serializing_if = "Option::is_none")]
    pub starred: Option<String>,
    #[serde(rename = "@year", skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(rename = "@genre", skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(rename = "song", skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<ChildResponse>,
}

impl AlbumWithSongsID3Response {
    #[must_use]
    pub fn from_album_and_songs(album: &Album, songs: Vec<ChildResponse>) -> Self {
        Self {
            id: album.id.to_string(),
            name: album.name.clone(),
            artist: album.artist_name.clone(),
            artist_id: album.artist_id.map(|id| id.to_string()),
            cover_art: album.cover_art.clone(),
            song_count: album.song_count,
            duration: album.duration,
            play_count: Some(album.play_count),
            created: album
                .created_at
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
            starred: None,
            year: album.year,
            genre: album.genre.clone(),
            songs,
        }
    }

    #[must_use]
    pub fn from_album_and_songs_with_starred(
        album: &Album,
        songs: Vec<ChildResponse>,
        starred_at: Option<&NaiveDateTime>,
    ) -> Self {
        Self {
            id: album.id.to_string(),
            name: album.name.clone(),
            artist: album.artist_name.clone(),
            artist_id: album.artist_id.map(|id| id.to_string()),
            cover_art: album.cover_art.clone(),
            song_count: album.song_count,
            duration: album.duration,
            play_count: Some(album.play_count),
            created: album
                .created_at
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
            starred: starred_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
            year: album.year,
            genre: album.genre.clone(),
            songs,
        }
    }
}

/// Artist with albums response for getArtist.
#[derive(Debug, Serialize, Clone)]
pub struct ArtistWithAlbumsID3Response {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "@artistImageUrl", skip_serializing_if = "Option::is_none")]
    pub artist_image_url: Option<String>,
    #[serde(rename = "@albumCount", skip_serializing_if = "Option::is_none")]
    pub album_count: Option<i32>,
    #[serde(rename = "@starred", skip_serializing_if = "Option::is_none")]
    pub starred: Option<String>,
    #[serde(rename = "@musicBrainzId", skip_serializing_if = "Option::is_none")]
    pub musicbrainz_id: Option<String>,
    #[serde(rename = "@sortName", skip_serializing_if = "Option::is_none")]
    pub sort_name: Option<String>,
    #[serde(rename = "album", skip_serializing_if = "Vec::is_empty")]
    pub albums: Vec<AlbumID3Response>,
}

impl ArtistWithAlbumsID3Response {
    #[must_use]
    pub fn from_artist_and_albums(artist: &Artist, albums: Vec<AlbumID3Response>) -> Self {
        Self {
            id: artist.id.to_string(),
            name: artist.name.clone(),
            cover_art: artist.cover_art.clone(),
            artist_image_url: artist.artist_image_url.clone(),
            album_count: Some(i32::try_from(albums.len()).unwrap_or(0)),
            starred: None,
            musicbrainz_id: artist.musicbrainz_id.clone(),
            sort_name: artist.sort_name.clone(),
            albums,
        }
    }

    #[must_use]
    pub fn from_artist_and_albums_with_starred(
        artist: &Artist,
        albums: Vec<AlbumID3Response>,
        starred_at: Option<&NaiveDateTime>,
    ) -> Self {
        Self {
            id: artist.id.to_string(),
            name: artist.name.clone(),
            cover_art: artist.cover_art.clone(),
            artist_image_url: artist.artist_image_url.clone(),
            album_count: Some(i32::try_from(albums.len()).unwrap_or(0)),
            starred: starred_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
            musicbrainz_id: artist.musicbrainz_id.clone(),
            sort_name: artist.sort_name.clone(),
            albums,
        }
    }
}

/// New music folder for insertion.
#[derive(Debug, Clone)]
pub struct NewMusicFolder {
    pub name: String,
    pub path: String,
    pub enabled: bool,
}

impl NewMusicFolder {
    pub fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            enabled: true,
        }
    }
}

/// New artist for insertion.
#[derive(Debug, Clone, Default)]
pub struct NewArtist {
    pub name: String,
    pub sort_name: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub cover_art: Option<String>,
    pub artist_image_url: Option<String>,
}

impl NewArtist {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}

/// New album for insertion.
#[derive(Debug, Clone, Default)]
pub struct NewAlbum {
    pub name: String,
    pub sort_name: Option<String>,
    pub artist_id: Option<i32>,
    pub artist_name: Option<String>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_art: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub duration: i32,
    pub song_count: i32,
}

impl NewAlbum {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Default::default()
        }
    }
}

/// New song for insertion.
#[derive(Debug, Clone)]
pub struct NewSong {
    pub title: String,
    pub sort_name: Option<String>,
    pub album_id: Option<i32>,
    pub artist_id: Option<i32>,
    pub artist_name: Option<String>,
    pub album_name: Option<String>,
    pub music_folder_id: i32,
    pub path: String,
    pub parent_path: String,
    pub file_size: i64,
    pub content_type: String,
    pub suffix: String,
    pub duration: i32,
    pub bit_rate: Option<i32>,
    pub bit_depth: Option<i32>,
    pub sampling_rate: Option<i32>,
    pub channel_count: Option<i32>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub year: Option<i32>,
    pub genre: Option<String>,
    pub cover_art: Option<String>,
    pub musicbrainz_id: Option<String>,
}

// ============================================================================
// Response types for getAlbumList2, getGenres, search3
// ============================================================================

/// Album list response for getAlbumList2.
#[derive(Debug, Serialize, Clone)]
pub struct AlbumList2Response {
    #[serde(rename = "album", skip_serializing_if = "Vec::is_empty")]
    pub albums: Vec<AlbumID3Response>,
}

/// Genre response for getGenres.
#[derive(Debug, Serialize, Clone)]
pub struct GenreResponse {
    #[serde(rename = "@songCount")]
    pub song_count: i64,
    #[serde(rename = "@albumCount")]
    pub album_count: i64,
    #[serde(rename = "$text")]
    pub value: String,
}

/// Genres response for getGenres.
#[derive(Debug, Serialize, Clone)]
pub struct GenresResponse {
    #[serde(rename = "genre", skip_serializing_if = "Vec::is_empty")]
    pub genres: Vec<GenreResponse>,
}

/// Search result response for search3.
#[derive(Debug, Serialize, Clone)]
pub struct SearchResult3Response {
    #[serde(rename = "artist", skip_serializing_if = "Vec::is_empty")]
    pub artists: Vec<ArtistID3Response>,
    #[serde(rename = "album", skip_serializing_if = "Vec::is_empty")]
    pub albums: Vec<AlbumID3Response>,
    #[serde(rename = "song", skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<ChildResponse>,
}

// ============================================================================
// Response types for starred (getStarred2)
// ============================================================================

/// `ChildResponse` with starred timestamp for getStarred2.
#[derive(Debug, Serialize, Clone)]
pub struct StarredChildResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@parent", skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(rename = "@isDir")]
    pub is_dir: bool,
    #[serde(rename = "@title")]
    pub title: String,
    #[serde(rename = "@album", skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(rename = "@artist", skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(rename = "@track", skip_serializing_if = "Option::is_none")]
    pub track: Option<i32>,
    #[serde(rename = "@year", skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(rename = "@genre", skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "@size", skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(rename = "@contentType", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(rename = "@suffix", skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(rename = "@duration", skip_serializing_if = "Option::is_none")]
    pub duration: Option<i32>,
    #[serde(rename = "@bitRate", skip_serializing_if = "Option::is_none")]
    pub bit_rate: Option<i32>,
    #[serde(rename = "@bitDepth", skip_serializing_if = "Option::is_none")]
    pub bit_depth: Option<i32>,
    #[serde(rename = "@samplingRate", skip_serializing_if = "Option::is_none")]
    pub sampling_rate: Option<i32>,
    #[serde(rename = "@channelCount", skip_serializing_if = "Option::is_none")]
    pub channel_count: Option<i32>,
    #[serde(rename = "@path", skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(rename = "@playCount", skip_serializing_if = "Option::is_none")]
    pub play_count: Option<i32>,
    #[serde(rename = "@discNumber", skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<i32>,
    #[serde(rename = "@created", skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(rename = "@albumId", skip_serializing_if = "Option::is_none")]
    pub album_id: Option<String>,
    #[serde(rename = "@artistId", skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<String>,
    #[serde(rename = "@type", skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(rename = "@starred")]
    pub starred: String,
}

impl StarredChildResponse {
    #[must_use]
    pub fn from_song_and_starred(song: &Song, starred_at: &chrono::NaiveDateTime) -> Self {
        Self {
            id: song.id.to_string(),
            parent: song.album_id.map(|id| id.to_string()),
            is_dir: false,
            title: song.title.clone(),
            album: song.album_name.clone(),
            artist: song.artist_name.clone(),
            track: song.track_number,
            year: song.year,
            genre: song.genre.clone(),
            cover_art: song.cover_art.clone(),
            size: Some(song.file_size),
            content_type: Some(song.content_type.clone()),
            suffix: Some(song.suffix.clone()),
            duration: Some(song.duration),
            bit_rate: song.bit_rate,
            bit_depth: song.bit_depth,
            sampling_rate: song.sampling_rate,
            channel_count: song.channel_count,
            path: Some(song.path.clone()),
            play_count: Some(song.play_count),
            disc_number: song.disc_number,
            created: Some(song.created_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
            album_id: song.album_id.map(|id| id.to_string()),
            artist_id: song.artist_id.map(|id| id.to_string()),
            media_type: Some("music".to_string()),
            starred: starred_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
        }
    }
}

/// `ArtistID3Response` with starred timestamp for getStarred2.
#[derive(Debug, Serialize, Clone)]
pub struct StarredArtistID3Response {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "@artistImageUrl", skip_serializing_if = "Option::is_none")]
    pub artist_image_url: Option<String>,
    #[serde(rename = "@albumCount", skip_serializing_if = "Option::is_none")]
    pub album_count: Option<i32>,
    #[serde(rename = "@starred")]
    pub starred: String,
    #[serde(rename = "@musicBrainzId", skip_serializing_if = "Option::is_none")]
    pub musicbrainz_id: Option<String>,
    #[serde(rename = "@sortName", skip_serializing_if = "Option::is_none")]
    pub sort_name: Option<String>,
}

impl StarredArtistID3Response {
    #[must_use]
    pub fn from_artist_and_starred(
        artist: &Artist,
        album_count: Option<i32>,
        starred_at: &chrono::NaiveDateTime,
    ) -> Self {
        Self {
            id: artist.id.to_string(),
            name: artist.name.clone(),
            cover_art: artist.cover_art.clone(),
            artist_image_url: artist.artist_image_url.clone(),
            album_count,
            starred: starred_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            musicbrainz_id: artist.musicbrainz_id.clone(),
            sort_name: artist.sort_name.clone(),
        }
    }
}

/// `AlbumID3Response` with starred timestamp for getStarred2.
#[derive(Debug, Serialize, Clone)]
pub struct StarredAlbumID3Response {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@artist", skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(rename = "@artistId", skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<String>,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "@songCount")]
    pub song_count: i32,
    #[serde(rename = "@duration")]
    pub duration: i32,
    #[serde(rename = "@playCount", skip_serializing_if = "Option::is_none")]
    pub play_count: Option<i32>,
    #[serde(rename = "@created")]
    pub created: String,
    #[serde(rename = "@starred")]
    pub starred: String,
    #[serde(rename = "@year", skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(rename = "@genre", skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
}

impl StarredAlbumID3Response {
    #[must_use]
    pub fn from_album_and_starred(album: &Album, starred_at: &chrono::NaiveDateTime) -> Self {
        Self {
            id: album.id.to_string(),
            name: album.name.clone(),
            artist: album.artist_name.clone(),
            artist_id: album.artist_id.map(|id| id.to_string()),
            cover_art: album.cover_art.clone(),
            song_count: album.song_count,
            duration: album.duration,
            play_count: Some(album.play_count),
            created: album
                .created_at
                .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                .to_string(),
            starred: starred_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string(),
            year: album.year,
            genre: album.genre.clone(),
        }
    }
}

/// Starred2 response for getStarred2.
#[derive(Debug, Serialize, Clone)]
pub struct Starred2Response {
    #[serde(rename = "artist", skip_serializing_if = "Vec::is_empty")]
    pub artists: Vec<StarredArtistID3Response>,
    #[serde(rename = "album", skip_serializing_if = "Vec::is_empty")]
    pub albums: Vec<StarredAlbumID3Response>,
    #[serde(rename = "song", skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<StarredChildResponse>,
}

// ============================================================================
// Response types for getNowPlaying
// ============================================================================

/// Now playing entry response for getNowPlaying.
#[derive(Debug, Serialize, Clone)]
pub struct NowPlayingEntryResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@parent", skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(rename = "@isDir")]
    pub is_dir: bool,
    #[serde(rename = "@title")]
    pub title: String,
    #[serde(rename = "@album", skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(rename = "@artist", skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(rename = "@track", skip_serializing_if = "Option::is_none")]
    pub track: Option<i32>,
    #[serde(rename = "@year", skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(rename = "@genre", skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "@size", skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(rename = "@contentType", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(rename = "@suffix", skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(rename = "@duration", skip_serializing_if = "Option::is_none")]
    pub duration: Option<i32>,
    #[serde(rename = "@bitRate", skip_serializing_if = "Option::is_none")]
    pub bit_rate: Option<i32>,
    #[serde(rename = "@path", skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(rename = "@albumId", skip_serializing_if = "Option::is_none")]
    pub album_id: Option<String>,
    #[serde(rename = "@artistId", skip_serializing_if = "Option::is_none")]
    pub artist_id: Option<String>,
    #[serde(rename = "@type", skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    // Now playing specific fields
    #[serde(rename = "@username")]
    pub username: String,
    #[serde(rename = "@minutesAgo")]
    pub minutes_ago: i32,
    #[serde(rename = "@playerId", skip_serializing_if = "Option::is_none")]
    pub player_id: Option<String>,
}

impl NowPlayingEntryResponse {
    #[must_use]
    pub fn from_now_playing(
        song: &Song,
        username: String,
        minutes_ago: i32,
        player_id: Option<String>,
    ) -> Self {
        Self {
            id: song.id.to_string(),
            parent: song.album_id.map(|id| id.to_string()),
            is_dir: false,
            title: song.title.clone(),
            album: song.album_name.clone(),
            artist: song.artist_name.clone(),
            track: song.track_number,
            year: song.year,
            genre: song.genre.clone(),
            cover_art: song.cover_art.clone(),
            size: Some(song.file_size),
            content_type: Some(song.content_type.clone()),
            suffix: Some(song.suffix.clone()),
            duration: Some(song.duration),
            bit_rate: song.bit_rate,
            path: Some(song.path.clone()),
            album_id: song.album_id.map(|id| id.to_string()),
            artist_id: song.artist_id.map(|id| id.to_string()),
            media_type: Some("music".to_string()),
            username,
            minutes_ago,
            player_id,
        }
    }
}

/// Now playing response for getNowPlaying.
#[derive(Debug, Serialize, Clone)]
pub struct NowPlayingResponse {
    #[serde(rename = "entry", skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<NowPlayingEntryResponse>,
}

// ============================================================================
// Response types for getRandomSongs and getSongsByGenre
// ============================================================================

/// Random songs response for getRandomSongs.
#[derive(Debug, Serialize, Clone)]
pub struct RandomSongsResponse {
    #[serde(rename = "song", skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<ChildResponse>,
}

/// Songs by genre response for getSongsByGenre.
#[derive(Debug, Serialize, Clone)]
pub struct SongsByGenreResponse {
    #[serde(rename = "song", skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<ChildResponse>,
}

// ============================================================================
// Response types for playlists
// ============================================================================

/// Playlist response for getPlaylists.
#[derive(Debug, Serialize, Clone)]
pub struct PlaylistResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@comment", skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(rename = "@owner")]
    pub owner: String,
    #[serde(rename = "@public")]
    pub public: bool,
    #[serde(rename = "@songCount")]
    pub song_count: i32,
    #[serde(rename = "@duration")]
    pub duration: i32,
    #[serde(rename = "@created")]
    pub created: String,
    #[serde(rename = "@changed")]
    pub changed: String,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
}

/// Playlists response for getPlaylists.
#[derive(Debug, Serialize, Clone)]
pub struct PlaylistsResponse {
    #[serde(rename = "playlist", skip_serializing_if = "Vec::is_empty")]
    pub playlists: Vec<PlaylistResponse>,
}

/// Playlist with songs response for getPlaylist.
#[derive(Debug, Serialize, Clone)]
pub struct PlaylistWithSongsResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@comment", skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    #[serde(rename = "@owner")]
    pub owner: String,
    #[serde(rename = "@public")]
    pub public: bool,
    #[serde(rename = "@songCount")]
    pub song_count: i32,
    #[serde(rename = "@duration")]
    pub duration: i32,
    #[serde(rename = "@created")]
    pub created: String,
    #[serde(rename = "@changed")]
    pub changed: String,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "entry", skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<ChildResponse>,
}

// ============================================================================
// Response types for play queue
// ============================================================================

/// Play queue response for getPlayQueue.
#[derive(Debug, Serialize, Clone)]
pub struct PlayQueueResponse {
    #[serde(rename = "@current", skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    #[serde(rename = "@position", skip_serializing_if = "Option::is_none")]
    pub position: Option<i64>,
    #[serde(rename = "@username")]
    pub username: String,
    #[serde(rename = "@changed")]
    pub changed: String,
    #[serde(rename = "@changedBy", skip_serializing_if = "Option::is_none")]
    pub changed_by: Option<String>,
    #[serde(rename = "entry", skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<ChildResponse>,
}

/// Play queue by index response for getPlayQueueByIndex (`OpenSubsonic`).
/// Uses currentIndex instead of current (song ID).
#[derive(Debug, Serialize, Clone)]
pub struct PlayQueueByIndexResponse {
    #[serde(rename = "@currentIndex", skip_serializing_if = "Option::is_none")]
    pub current_index: Option<i32>,
    #[serde(rename = "@position", skip_serializing_if = "Option::is_none")]
    pub position: Option<i64>,
    #[serde(rename = "@username")]
    pub username: String,
    #[serde(rename = "@changed")]
    pub changed: String,
    #[serde(rename = "@changedBy", skip_serializing_if = "Option::is_none")]
    pub changed_by: Option<String>,
    #[serde(rename = "entry", skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<ChildResponse>,
}

/// Token info response for tokenInfo (`OpenSubsonic`).
/// Returns information about the API key used for authentication.
#[derive(Debug, Serialize, Clone)]
pub struct TokenInfoResponse {
    #[serde(rename = "@username")]
    pub username: String,
}

// ============================================================================
// Response types for getArtistInfo2, getAlbumInfo2, getSimilarSongs2, getTopSongs
// ============================================================================

/// Artist info response for getArtistInfo2.
#[derive(Debug, Serialize, Clone)]
pub struct ArtistInfo2Response {
    #[serde(rename = "@biography", skip_serializing_if = "Option::is_none")]
    pub biography: Option<String>,
    #[serde(rename = "@musicBrainzId", skip_serializing_if = "Option::is_none")]
    pub musicbrainz_id: Option<String>,
    #[serde(rename = "@lastFmUrl", skip_serializing_if = "Option::is_none")]
    pub last_fm_url: Option<String>,
    #[serde(rename = "@smallImageUrl", skip_serializing_if = "Option::is_none")]
    pub small_image_url: Option<String>,
    #[serde(rename = "@mediumImageUrl", skip_serializing_if = "Option::is_none")]
    pub medium_image_url: Option<String>,
    #[serde(rename = "@largeImageUrl", skip_serializing_if = "Option::is_none")]
    pub large_image_url: Option<String>,
    #[serde(rename = "similarArtist", skip_serializing_if = "Vec::is_empty")]
    pub similar_artists: Vec<ArtistID3Response>,
}

impl ArtistInfo2Response {
    /// Create an empty artist info response (stub).
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            biography: None,
            musicbrainz_id: None,
            last_fm_url: None,
            small_image_url: None,
            medium_image_url: None,
            large_image_url: None,
            similar_artists: Vec::new(),
        }
    }

    /// Create an artist info response with `musicbrainz_id` from the artist.
    #[must_use]
    pub fn from_artist(artist: &Artist) -> Self {
        Self {
            biography: None,
            musicbrainz_id: artist.musicbrainz_id.clone(),
            last_fm_url: None,
            small_image_url: artist.artist_image_url.clone(),
            medium_image_url: artist.artist_image_url.clone(),
            large_image_url: artist.artist_image_url.clone(),
            similar_artists: Vec::new(),
        }
    }
}

/// Album info response for getAlbumInfo2.
#[derive(Debug, Serialize, Clone)]
pub struct AlbumInfoResponse {
    #[serde(rename = "@notes", skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    #[serde(rename = "@musicBrainzId", skip_serializing_if = "Option::is_none")]
    pub musicbrainz_id: Option<String>,
    #[serde(rename = "@lastFmUrl", skip_serializing_if = "Option::is_none")]
    pub last_fm_url: Option<String>,
    #[serde(rename = "@smallImageUrl", skip_serializing_if = "Option::is_none")]
    pub small_image_url: Option<String>,
    #[serde(rename = "@mediumImageUrl", skip_serializing_if = "Option::is_none")]
    pub medium_image_url: Option<String>,
    #[serde(rename = "@largeImageUrl", skip_serializing_if = "Option::is_none")]
    pub large_image_url: Option<String>,
}

impl AlbumInfoResponse {
    /// Create an empty album info response (stub).
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            notes: None,
            musicbrainz_id: None,
            last_fm_url: None,
            small_image_url: None,
            medium_image_url: None,
            large_image_url: None,
        }
    }

    /// Create an album info response with data from the album.
    #[must_use]
    pub fn from_album(album: &Album) -> Self {
        Self {
            notes: None,
            musicbrainz_id: album.musicbrainz_id.clone(),
            last_fm_url: None,
            small_image_url: None,
            medium_image_url: None,
            large_image_url: None,
        }
    }
}

/// Similar songs response for getSimilarSongs2.
#[derive(Debug, Serialize, Clone)]
pub struct SimilarSongs2Response {
    #[serde(rename = "song", skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<ChildResponse>,
}

/// Top songs response for getTopSongs.
#[derive(Debug, Serialize, Clone)]
pub struct TopSongsResponse {
    #[serde(rename = "song", skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<ChildResponse>,
}

/// Lyrics response for getLyrics (original Subsonic API).
#[derive(Debug, Serialize, Clone)]
pub struct LyricsResponse {
    #[serde(rename = "@artist", skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(rename = "@title", skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(rename = "$text", skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl LyricsResponse {
    /// Create an empty lyrics response.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            artist: None,
            title: None,
            value: None,
        }
    }

    /// Create a lyrics response with data.
    #[must_use]
    pub const fn new(
        artist: Option<String>,
        title: Option<String>,
        lyrics: Option<String>,
    ) -> Self {
        Self {
            artist,
            title,
            value: lyrics,
        }
    }
}

// ============================================================================
// OpenSubsonic Structured Lyrics Types (for getLyricsBySongId)
// ============================================================================

/// A single line of lyrics, optionally with a start time for synchronized lyrics.
#[derive(Debug, Serialize, Clone)]
pub struct LyricLine {
    /// Start time in milliseconds from track start. Omit if unsynced.
    #[serde(rename = "@start", skip_serializing_if = "Option::is_none")]
    pub start: Option<i64>,
    /// The actual text of the lyric line.
    #[serde(rename = "$text")]
    pub value: String,
}

impl LyricLine {
    /// Create a new synced lyric line with a start time.
    pub fn synced(start: i64, value: impl Into<String>) -> Self {
        Self {
            start: Some(start),
            value: value.into(),
        }
    }

    /// Create a new unsynced lyric line without a start time.
    pub fn unsynced(value: impl Into<String>) -> Self {
        Self {
            start: None,
            value: value.into(),
        }
    }
}

/// Structured lyrics for a song, supporting synchronized lyrics and multiple languages.
#[derive(Debug, Serialize, Clone)]
pub struct StructuredLyrics {
    /// The artist name to display. May differ from the song's artist (e.g., localized).
    #[serde(rename = "@displayArtist", skip_serializing_if = "Option::is_none")]
    pub display_artist: Option<String>,
    /// The song title to display. May differ from the song's title (e.g., localized).
    #[serde(rename = "@displayTitle", skip_serializing_if = "Option::is_none")]
    pub display_title: Option<String>,
    /// ISO 639 language code (e.g., "eng", "jpn"). Use "und" or "xxx" for unknown.
    #[serde(rename = "@lang")]
    pub lang: String,
    /// Offset in milliseconds. Positive = lyrics appear sooner, negative = later.
    #[serde(rename = "@offset", skip_serializing_if = "Option::is_none")]
    pub offset: Option<i64>,
    /// Whether the lyrics are synchronized (have timestamps).
    #[serde(rename = "@synced")]
    pub synced: bool,
    /// The lyric lines, ordered by start time (synced) or appearance (unsynced).
    #[serde(rename = "line")]
    pub lines: Vec<LyricLine>,
}

impl StructuredLyrics {
    /// Create new unsynced lyrics.
    pub fn unsynced(lang: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            display_artist: None,
            display_title: None,
            lang: lang.into(),
            offset: None,
            synced: false,
            lines: lines.into_iter().map(LyricLine::unsynced).collect(),
        }
    }

    /// Create new synced lyrics.
    pub fn synced(lang: impl Into<String>, lines: Vec<(i64, String)>) -> Self {
        Self {
            display_artist: None,
            display_title: None,
            lang: lang.into(),
            offset: None,
            synced: true,
            lines: lines
                .into_iter()
                .map(|(start, value)| LyricLine::synced(start, value))
                .collect(),
        }
    }

    /// Set the display artist.
    #[must_use]
    pub fn with_display_artist(mut self, artist: impl Into<String>) -> Self {
        self.display_artist = Some(artist.into());
        self
    }

    /// Set the display title.
    #[must_use]
    pub fn with_display_title(mut self, title: impl Into<String>) -> Self {
        self.display_title = Some(title.into());
        self
    }

    /// Set the offset.
    #[must_use]
    pub const fn with_offset(mut self, offset: i64) -> Self {
        self.offset = Some(offset);
        self
    }
}

/// Lyrics list response for getLyricsBySongId (`OpenSubsonic` extension).
#[derive(Debug, Serialize, Clone)]
pub struct LyricsListResponse {
    /// Array of structured lyrics. May have multiple entries for different languages.
    #[serde(rename = "structuredLyrics", skip_serializing_if = "Vec::is_empty")]
    pub structured_lyrics: Vec<StructuredLyrics>,
}

impl LyricsListResponse {
    /// Create an empty lyrics list response.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            structured_lyrics: Vec::new(),
        }
    }

    /// Create a lyrics list response with structured lyrics.
    #[must_use]
    pub const fn new(structured_lyrics: Vec<StructuredLyrics>) -> Self {
        Self { structured_lyrics }
    }
}

// ============================================================================
// Response types for getMusicDirectory (non-ID3 folder browsing)
// ============================================================================

/// Directory response for getMusicDirectory.
#[derive(Debug, Serialize, Clone)]
pub struct DirectoryResponse {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@parent", skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(rename = "@name")]
    pub name: String,
    #[serde(rename = "@starred", skip_serializing_if = "Option::is_none")]
    pub starred: Option<String>,
    #[serde(rename = "@playCount", skip_serializing_if = "Option::is_none")]
    pub play_count: Option<i32>,
    #[serde(rename = "child", skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ChildResponse>,
}

impl DirectoryResponse {
    /// Create a directory response from a music folder.
    #[must_use]
    pub fn from_music_folder(folder: &MusicFolder, children: Vec<ChildResponse>) -> Self {
        Self {
            id: folder.id.to_string(),
            parent: None,
            name: folder.name.clone(),
            starred: None,
            play_count: None,
            children,
        }
    }

    /// Create a directory response from an artist.
    #[must_use]
    pub fn from_artist(artist: &Artist, children: Vec<ChildResponse>) -> Self {
        Self {
            id: artist.id.to_string(),
            parent: None,
            name: artist.name.clone(),
            starred: None,
            play_count: None,
            children,
        }
    }

    /// Create a directory response from an album.
    #[must_use]
    pub fn from_album(album: &Album, children: Vec<ChildResponse>) -> Self {
        Self {
            id: album.id.to_string(),
            parent: album.artist_id.map(|id| id.to_string()),
            name: album.name.clone(),
            starred: None,
            play_count: Some(album.play_count),
            children,
        }
    }
}

impl ChildResponse {
    /// Create a child response representing an artist (as directory).
    #[must_use]
    pub fn from_artist_as_dir(artist: &Artist) -> Self {
        Self {
            id: artist.id.to_string(),
            parent: None,
            is_dir: true,
            title: artist.name.clone(),
            album: None,
            artist: Some(artist.name.clone()),
            track: None,
            year: None,
            genre: None,
            cover_art: artist.cover_art.clone(),
            size: None,
            content_type: None,
            suffix: None,
            duration: None,
            bit_rate: None,
            bit_depth: None,
            sampling_rate: None,
            channel_count: None,
            path: None,
            play_count: None,
            disc_number: None,
            created: Some(
                artist
                    .created_at
                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                    .to_string(),
            ),
            album_id: None,
            artist_id: Some(artist.id.to_string()),
            media_type: None,
            starred: None,
        }
    }

    /// Create a child response representing an album (as directory).
    #[must_use]
    pub fn from_album_as_dir(album: &Album) -> Self {
        Self {
            id: album.id.to_string(),
            parent: album.artist_id.map(|id| id.to_string()),
            is_dir: true,
            title: album.name.clone(),
            album: Some(album.name.clone()),
            artist: album.artist_name.clone(),
            track: None,
            year: album.year,
            genre: album.genre.clone(),
            cover_art: album.cover_art.clone(),
            size: None,
            content_type: None,
            suffix: None,
            duration: Some(album.duration),
            bit_rate: None,
            bit_depth: None,
            sampling_rate: None,
            channel_count: None,
            path: None,
            play_count: Some(album.play_count),
            disc_number: None,
            created: Some(
                album
                    .created_at
                    .format("%Y-%m-%dT%H:%M:%S%.3fZ")
                    .to_string(),
            ),
            album_id: Some(album.id.to_string()),
            artist_id: album.artist_id.map(|id| id.to_string()),
            media_type: None,
            starred: None,
        }
    }
}

// ============================================================================
// Response types for getAlbumList (non-ID3)
// ============================================================================

/// Album list response for getAlbumList (non-ID3 version).
#[derive(Debug, Serialize, Clone)]
pub struct AlbumListResponse {
    #[serde(rename = "album", skip_serializing_if = "Vec::is_empty")]
    pub albums: Vec<ChildResponse>,
}

// ============================================================================
// Response types for getStarred (non-ID3)
// ============================================================================

/// Starred response for getStarred (non-ID3).
#[derive(Debug, Serialize, Clone)]
pub struct StarredResponse {
    #[serde(rename = "artist", skip_serializing_if = "Vec::is_empty")]
    pub artists: Vec<ArtistResponse>,
    #[serde(rename = "album", skip_serializing_if = "Vec::is_empty")]
    pub albums: Vec<ChildResponse>,
    #[serde(rename = "song", skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<ChildResponse>,
}

impl ArtistResponse {
    /// Create an artist response with starred timestamp.
    #[must_use]
    pub fn from_artist_with_starred(
        artist: &Artist,
        starred_at: Option<&chrono::NaiveDateTime>,
    ) -> Self {
        Self {
            id: artist.id.to_string(),
            name: artist.name.clone(),
            artist_image_url: artist.artist_image_url.clone(),
            starred: starred_at.map(|dt| dt.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
            user_rating: None,
            average_rating: None,
        }
    }
}

// ============================================================================
// Response types for search2 (older search API)
// ============================================================================

/// Search result response for search2 (non-ID3).
#[derive(Debug, Serialize, Clone)]
pub struct SearchResult2Response {
    #[serde(rename = "artist", skip_serializing_if = "Vec::is_empty")]
    pub artists: Vec<ArtistResponse>,
    #[serde(rename = "album", skip_serializing_if = "Vec::is_empty")]
    pub albums: Vec<ChildResponse>,
    #[serde(rename = "song", skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<ChildResponse>,
}

// ============================================================================
// Response types for search (legacy search API)
// ============================================================================

/// Match entry for legacy search.
#[derive(Debug, Serialize, Clone)]
pub struct SearchMatch {
    #[serde(rename = "@id")]
    pub id: String,
    #[serde(rename = "@parent", skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    #[serde(rename = "@isDir")]
    pub is_dir: bool,
    #[serde(rename = "@title")]
    pub title: String,
    #[serde(rename = "@album", skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(rename = "@artist", skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(rename = "@track", skip_serializing_if = "Option::is_none")]
    pub track: Option<i32>,
    #[serde(rename = "@year", skip_serializing_if = "Option::is_none")]
    pub year: Option<i32>,
    #[serde(rename = "@genre", skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(rename = "@coverArt", skip_serializing_if = "Option::is_none")]
    pub cover_art: Option<String>,
    #[serde(rename = "@size", skip_serializing_if = "Option::is_none")]
    pub size: Option<i64>,
    #[serde(rename = "@contentType", skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(rename = "@suffix", skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    #[serde(rename = "@duration", skip_serializing_if = "Option::is_none")]
    pub duration: Option<i32>,
    #[serde(rename = "@bitRate", skip_serializing_if = "Option::is_none")]
    pub bit_rate: Option<i32>,
    #[serde(rename = "@path", skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(rename = "@created", skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
}

impl From<&Song> for SearchMatch {
    fn from(song: &Song) -> Self {
        Self {
            id: song.id.to_string(),
            parent: song.album_id.map(|id| id.to_string()),
            is_dir: false,
            title: song.title.clone(),
            album: song.album_name.clone(),
            artist: song.artist_name.clone(),
            track: song.track_number,
            year: song.year,
            genre: song.genre.clone(),
            cover_art: song.cover_art.clone(),
            size: Some(song.file_size),
            content_type: Some(song.content_type.clone()),
            suffix: Some(song.suffix.clone()),
            duration: Some(song.duration),
            bit_rate: song.bit_rate,
            path: Some(song.path.clone()),
            created: Some(song.created_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
        }
    }
}

/// Search result response for legacy search.
#[derive(Debug, Serialize, Clone)]
pub struct SearchResultResponse {
    #[serde(rename = "@offset")]
    pub offset: i64,
    #[serde(rename = "@totalHits")]
    pub total_hits: i64,
    #[serde(rename = "match", skip_serializing_if = "Vec::is_empty")]
    pub matches: Vec<SearchMatch>,
}

// ============================================================================
// Response types for getArtistInfo (non-ID3)
// ============================================================================

/// Artist info response for getArtistInfo (non-ID3).
#[derive(Debug, Serialize, Clone)]
pub struct ArtistInfoResponse {
    #[serde(rename = "@biography", skip_serializing_if = "Option::is_none")]
    pub biography: Option<String>,
    #[serde(rename = "@musicBrainzId", skip_serializing_if = "Option::is_none")]
    pub musicbrainz_id: Option<String>,
    #[serde(rename = "@lastFmUrl", skip_serializing_if = "Option::is_none")]
    pub last_fm_url: Option<String>,
    #[serde(rename = "@smallImageUrl", skip_serializing_if = "Option::is_none")]
    pub small_image_url: Option<String>,
    #[serde(rename = "@mediumImageUrl", skip_serializing_if = "Option::is_none")]
    pub medium_image_url: Option<String>,
    #[serde(rename = "@largeImageUrl", skip_serializing_if = "Option::is_none")]
    pub large_image_url: Option<String>,
    #[serde(rename = "similarArtist", skip_serializing_if = "Vec::is_empty")]
    pub similar_artists: Vec<ArtistResponse>,
}

impl ArtistInfoResponse {
    /// Create an artist info response from an artist.
    #[must_use]
    pub fn from_artist(artist: &Artist) -> Self {
        Self {
            biography: None,
            musicbrainz_id: artist.musicbrainz_id.clone(),
            last_fm_url: None,
            small_image_url: artist.artist_image_url.clone(),
            medium_image_url: artist.artist_image_url.clone(),
            large_image_url: artist.artist_image_url.clone(),
            similar_artists: Vec::new(),
        }
    }
}

// ============================================================================
// Response types for getSimilarSongs (non-ID3)
// ============================================================================

/// Similar songs response for getSimilarSongs (non-ID3).
#[derive(Debug, Serialize, Clone)]
pub struct SimilarSongsResponse {
    #[serde(rename = "song", skip_serializing_if = "Vec::is_empty")]
    pub songs: Vec<ChildResponse>,
}
