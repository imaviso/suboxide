//! Last.fm API models and response types.

use serde::Deserialize;

/// Artist information from Last.fm.
#[derive(Debug, Clone, Deserialize)]
pub struct LastFmArtist {
    pub name: String,
    #[serde(rename = "mbid")]
    pub musicbrainz_id: Option<String>,
    pub url: Option<String>,
    #[serde(default)]
    pub image: Vec<LastFmImage>,
    #[serde(default)]
    pub bio: Option<LastFmBio>,
    #[serde(default)]
    pub similar: LastFmSimilarArtists,
}

/// Image from Last.fm with different sizes.
#[derive(Debug, Clone, Deserialize)]
pub struct LastFmImage {
    #[serde(rename = "#text")]
    pub url: String,
    pub size: String,
}

/// Artist biography from Last.fm.
#[derive(Debug, Clone, Deserialize)]
pub struct LastFmBio {
    pub summary: Option<String>,
    pub content: Option<String>,
}

/// Similar artists container from Last.fm.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct LastFmSimilarArtists {
    #[serde(default)]
    pub artist: Vec<LastFmSimilarArtist>,
}

/// Similar artist entry from Last.fm.
#[derive(Debug, Clone, Deserialize)]
pub struct LastFmSimilarArtist {
    pub name: String,
    pub url: Option<String>,
    #[serde(default)]
    pub image: Vec<LastFmImage>,
}

/// Last.fm API response wrapper.
#[derive(Debug, Clone, Deserialize)]
pub struct LastFmResponse<T> {
    pub artist: Option<T>,
    #[serde(rename = "@attr")]
    pub attr: Option<LastFmAttr>,
}

/// Last.fm response attributes.
#[derive(Debug, Clone, Deserialize)]
pub struct LastFmAttr {
    pub status: String,
}

/// Last.fm error response.
#[derive(Debug, Clone, Deserialize)]
pub struct LastFmError {
    pub error: i32,
    pub message: String,
}

/// Cache entry for Last.fm artist info.
#[derive(Debug, Clone)]
pub struct LastFmArtistCache {
    pub artist_id: i32,
    pub biography: Option<String>,
    pub last_fm_url: Option<String>,
    pub small_image_url: Option<String>,
    pub medium_image_url: Option<String>,
    pub large_image_url: Option<String>,
    pub similar_artists: Vec<String>,
    pub updated_at: chrono::NaiveDateTime,
}

/// Convert Last.fm images to our format.
#[must_use]
pub fn extract_image_urls(
    images: &[LastFmImage],
) -> (Option<String>, Option<String>, Option<String>) {
    let mut small = None;
    let mut medium = None;
    let mut large = None;

    for image in images {
        match image.size.as_str() {
            "small" => small = Some(image.url.clone()),
            "medium" => medium = Some(image.url.clone()),
            "large" => large = Some(image.url.clone()),
            _ => {}
        }
    }

    // If we have larger sizes but no smaller ones, use the larger ones
    if small.is_none() && medium.is_some() {
        small.clone_from(&medium);
    }
    if medium.is_none() && large.is_some() {
        medium.clone_from(&large);
    }
    if large.is_none() && medium.is_some() {
        large.clone_from(&medium);
    }

    (small, medium, large)
}

/// Extract biography content (preferring full content over summary).
#[must_use]
pub fn extract_biography(bio: &Option<LastFmBio>) -> Option<String> {
    bio.as_ref().and_then(|b| {
        b.content
            .clone()
            .filter(|c| !c.is_empty())
            .or_else(|| b.summary.clone().filter(|s| !s.is_empty()))
    })
}
