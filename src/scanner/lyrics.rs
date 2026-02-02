//! Lyrics extraction from audio files.
//!
//! Extracts embedded lyrics from audio files using the lofty crate.
//! Supports both synchronized (LRC/SYLT) and unsynchronized lyrics.

use std::path::Path;

use lofty::file::TaggedFileExt;
use lofty::tag::ItemKey;

/// Extracted lyrics from an audio file.
#[derive(Debug, Clone)]
pub struct ExtractedLyrics {
    /// The lyrics text (may contain LRC timestamps if synced).
    pub text: String,
    /// Whether the lyrics are synchronized (have timestamps).
    pub synced: bool,
    /// Language code if available (e.g., "eng", "jpn").
    pub lang: Option<String>,
    /// Description/type of lyrics if available.
    pub description: Option<String>,
}

/// Parsed synchronized lyric line.
#[derive(Debug, Clone)]
pub struct SyncedLine {
    /// Start time in milliseconds.
    pub start_ms: i64,
    /// The lyric text.
    pub text: String,
}

/// Extract lyrics from an audio file.
///
/// Returns a list of extracted lyrics (may have multiple for different languages).
#[must_use]
pub fn extract_lyrics(path: &Path) -> Vec<ExtractedLyrics> {
    let mut results = Vec::new();

    let Ok(tagged_file) = lofty::read_from_path(path) else {
        return results;
    };

    // Try primary tag first, then any available tag
    let Some(tag) = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())
    else {
        return results;
    };

    // Try to get unsynchronized lyrics (USLT in ID3, LYRICS in Vorbis)
    // ItemKey::Lyrics is the standard key for unsynchronized lyrics
    if let Some(lyrics_text) = tag.get_string(&ItemKey::Lyrics)
        && !lyrics_text.trim().is_empty()
    {
        // Check if the "unsynced" lyrics actually contain LRC timestamps
        let (text, synced) = if looks_like_lrc(lyrics_text) {
            (lyrics_text.to_string(), true)
        } else {
            (lyrics_text.to_string(), false)
        };

        results.push(ExtractedLyrics {
            text,
            synced,
            lang: None,
            description: None,
        });
    }

    // Also check for synced lyrics stored in a different field
    // Some taggers use "SYNCEDLYRICS" or similar custom fields
    if let Some(synced_text) = tag.get_string(&ItemKey::Unknown("SYNCEDLYRICS".to_string()))
        && !synced_text.trim().is_empty()
        && !results.iter().any(|l| l.text == synced_text)
    {
        results.push(ExtractedLyrics {
            text: synced_text.to_string(),
            synced: true,
            lang: None,
            description: Some("synced".to_string()),
        });
    }

    results
}

/// Check if text looks like LRC format (has timestamps like [00:00.00]).
fn looks_like_lrc(text: &str) -> bool {
    // LRC format: [mm:ss.xx] or [mm:ss:xx] or [mm:ss]
    text.lines()
        .take(10) // Check first 10 lines
        .any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('[')
                && trimmed.len() > 3
                && trimmed[1..]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_digit())
        })
}

/// Parse LRC formatted lyrics into synchronized lines.
///
/// LRC format: [mm:ss.xx]text or [mm:ss:xx]text or [mm:ss]text
#[must_use]
pub fn parse_lrc(text: &str) -> Vec<SyncedLine> {
    let mut lines = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Parse all timestamps at the start of the line
        // Some LRC files have multiple timestamps: [00:01.00][00:02.00]text
        let mut remaining = trimmed;
        let mut timestamps = Vec::new();

        while remaining.starts_with('[') {
            if let Some(end) = remaining.find(']') {
                let timestamp_str = &remaining[1..end];
                if let Some(ms) = parse_lrc_timestamp(timestamp_str) {
                    timestamps.push(ms);
                } else {
                    // Not a timestamp (might be metadata like [ar:Artist])
                    break;
                }
                remaining = &remaining[end + 1..];
            } else {
                break;
            }
        }

        // The remaining text is the lyric line
        let lyric_text = remaining.trim();

        // Create a line for each timestamp
        for start_ms in timestamps {
            lines.push(SyncedLine {
                start_ms,
                text: lyric_text.to_string(),
            });
        }
    }

    // Sort by timestamp
    lines.sort_by_key(|l| l.start_ms);
    lines
}

/// Parse an LRC timestamp into milliseconds.
///
/// Formats: mm:ss.xx, mm:ss:xx, mm:ss, m:ss.xx
fn parse_lrc_timestamp(s: &str) -> Option<i64> {
    let parts: Vec<&str> = s.split([':', '.']).collect();

    match parts.len() {
        2 => {
            // mm:ss
            let mins: i64 = parts[0].parse().ok()?;
            let secs: i64 = parts[1].parse().ok()?;
            Some(mins * 60_000 + secs * 1000)
        }
        3 => {
            // mm:ss.xx or mm:ss:xx
            let mins: i64 = parts[0].parse().ok()?;
            let secs: i64 = parts[1].parse().ok()?;
            let centis: i64 = parts[2].parse().ok()?;
            // Handle both centiseconds (xx) and milliseconds (xxx)
            let ms = if parts[2].len() == 2 {
                centis * 10
            } else {
                centis
            };
            Some(mins * 60_000 + secs * 1000 + ms)
        }
        _ => None,
    }
}

/// Parse unsynchronized lyrics into individual lines.
#[must_use]
pub fn parse_unsynced(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lrc_timestamp() {
        assert_eq!(parse_lrc_timestamp("00:00"), Some(0));
        assert_eq!(parse_lrc_timestamp("00:01"), Some(1000));
        assert_eq!(parse_lrc_timestamp("01:00"), Some(60_000));
        assert_eq!(parse_lrc_timestamp("01:30"), Some(90_000));
        assert_eq!(parse_lrc_timestamp("00:00.00"), Some(0));
        assert_eq!(parse_lrc_timestamp("00:01.50"), Some(1500));
        assert_eq!(parse_lrc_timestamp("02:30.75"), Some(150_750));
    }

    #[test]
    fn test_parse_lrc() {
        let lrc = "[00:00.00]First line\n[00:05.00]Second line\n[00:10.50]Third line";
        let lines = parse_lrc(lrc);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].start_ms, 0);
        assert_eq!(lines[0].text, "First line");
        assert_eq!(lines[1].start_ms, 5000);
        assert_eq!(lines[2].start_ms, 10500);
    }

    #[test]
    fn test_looks_like_lrc() {
        assert!(looks_like_lrc("[00:00.00]Hello"));
        assert!(looks_like_lrc("[01:30]Test"));
        assert!(!looks_like_lrc("Just plain text"));
        assert!(!looks_like_lrc("[ar:Artist Name]")); // metadata tag
    }
}
