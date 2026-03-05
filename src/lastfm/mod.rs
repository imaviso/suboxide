//! Last.fm integration module.
//!
//! This module provides:
//! - API client for scrobbling and fetching artist info
//! - Data models for Last.fm API responses
//!
//! To use Last.fm:
//! 1. Set `LASTFM_API_KEY` and `LASTFM_API_SECRET` environment variables
//! 2. Users configure their Last.fm session key (via CLI or future web UI)
//! 3. Scrobbles are automatically sent when configured
//! 4. Artist info is fetched and cached with TTL

pub mod client;
pub mod models;

#[doc(inline)]
pub use client::{LastFmClient, LastFmError};
#[doc(inline)]
pub use models::{LastFmArtist, LastFmArtistCache, extract_biography, extract_image_urls};
