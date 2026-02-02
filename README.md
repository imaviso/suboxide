# Subsonic

A lightweight, self-hosted music streaming server implementing some of the [Subsonic API](http://www.subsonic.org/pages/api.jsp) and [OpenSubsonic](https://opensubsonic.netlify.app/) extensions, written in Rust.

> **Note** This is a personal use project built to fit specific needs. As a result, not all endpoints from the full Subsonic API specification are implemented. It includes the most commonly used features for streaming music and managing libraries. For production use cases requiring comprehensive API coverage, consider using [Navidrome](https://www.navidrome.org/) or other established Subsonic implementations.

## Features

- **Subsonic API Compatible** - Works with any Subsonic-compatible client (DSub, Symfonium, Submariner, etc.)
- **OpenSubsonic Extensions** - Supports modern extensions like API key authentication and form POST
- **Last.fm Integration** - Automatic scrobbling, now playing updates, and artist info/image fetching
- **Fast & Lightweight** - Built with Rust, Axum, and SQLite for minimal resource usage
- **Easy Setup** - Single binary with SQLite database, no external dependencies
- **Music Library Scanning** - Automatically scans and indexes your music collection
- **User Management** - Multi-user support with role-based permissions

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/momoyaan/subsonic.git
cd subsonic

# Build with Cargo
cargo build --release

# The binary will be at target/release/subsonic
```

### With Nix

```bash
nix develop  # Enter development shell
cargo build --release
```

## Quick Start

```bash
# 1. Create an admin user
./subsonic create-user --username admin --password yourpassword --admin

# 2. Add your music folder
./subsonic add-folder --name "Music" --path /path/to/your/music

# 3. Scan your library
./subsonic scan

# 4. Start the server
./subsonic serve
```

The server will start on `http://localhost:4040` by default.

## Configuration

### Command Line Options

```
Usage: subsonic [OPTIONS] [COMMAND]

Commands:
  create-user       Create a new user
  generate-api-key  Generate an API key for a user
  revoke-api-key    Revoke (delete) an API key for a user
  show-api-key      Show a user's API key
  add-folder        Add a music folder
  list-folders      List all music folders
  remove-folder     Remove a music folder
  scan              Scan music folders for audio files
  serve             Start the server (default)

Options:
  -d, --database <FILE>  Database file path [default: subsonic.db]
  -p, --port <PORT>      Server port [default: 4040]
  -h, --help             Print help
```

### Last.fm Integration

To enable Last.fm features (scrobbling and metadata fetching), set your API credentials:

```bash
export LASTFM_API_KEY="your_api_key"
export LASTFM_API_SECRET="your_api_secret"
```

Then, link a user account to Last.fm:

```bash
# Follow the interactive prompt to authorize
./subsonic user link-lastfm --username your_username
```

### Environment Variables

- `RUST_LOG` - Set log level (e.g., `subsonic=debug,tower_http=debug`)
- `LASTFM_API_KEY` - Last.fm API Key
- `LASTFM_API_SECRET` - Last.fm API Secret

## API Endpoints

### Implemented (52 endpoints)

| Category | Endpoints |
|----------|-----------|
| **System** | `ping`, `getLicense`, `getOpenSubsonicExtensions` |
| **Browsing** | `getMusicFolders`, `getIndexes`, `getMusicDirectory`, `getArtists`, `getArtist`, `getAlbum`, `getSong`, `getAlbumList`, `getAlbumList2`, `getGenres`, `getArtistInfo`, `getArtistInfo2`, `getAlbumInfo`, `getAlbumInfo2`, `getSimilarSongs`, `getSimilarSongs2`, `getTopSongs`, `getRandomSongs`, `getSongsByGenre` |
| **Searching** | `search`, `search2`, `search3` |
| **Playlists** | `getPlaylists`, `getPlaylist`, `createPlaylist`, `updatePlaylist`, `deletePlaylist` |
| **Media Retrieval** | `stream`, `download`, `getCoverArt`, `getLyrics`, `getLyricsBySongId` |
| **Annotation** | `star`, `unstar`, `getStarred`, `getStarred2`, `scrobble`, `setRating`, `getNowPlaying` |
| **Bookmarks** | `getBookmarks` |
| **Play Queue** | `getPlayQueue`, `savePlayQueue` |
| **User Management** | `getUser`, `getUsers`, `createUser`, `updateUser`, `deleteUser`, `changePassword` |
| **Scanning** | `startScan`, `getScanStatus` |

### Authentication

The server supports three authentication methods:

1. **Token Authentication** (recommended) - MD5(password + salt) via `t` and `s` parameters
2. **API Key** (OpenSubsonic) - Via `apiKey` parameter
3. **Legacy Password** - Plain or hex-encoded via `p` parameter

## Supported Audio Formats

The scanner recognizes the following formats:
- FLAC, MP3, AAC/M4A, OGG/Opus, WAV, AIFF, WMA, APE, WavPack

## Client Compatibility

Tested with:
- [Symfonium](https://symfonium.app/) (Android)
- [DSub](https://github.com/daneren2005/Subsonic) (Android)
- [Submariner](https://submarinerapp.com/) (macOS)
- [Sonixd](https://github.com/jeffvli/sonixd) (Desktop)
- [Feishin](https://github.com/jeffvli/feishin) (Desktop)

## Development

```bash
# Enter nix development shell (includes Rust toolchain)
nix develop

# Run in development mode
cargo run -- serve

# Run tests
cargo test

# Check formatting
cargo fmt --check

# Run linter
cargo clippy
```

## Project Structure

```
src/
├── main.rs          # CLI and server entry point
├── lib.rs           # Library exports
├── api/             # Subsonic API implementation
│   ├── auth.rs      # Authentication middleware
│   ├── handlers/    # API endpoint handlers
│   └── response.rs  # Response formatting (XML/JSON)
├── db/              # Database layer (Diesel + SQLite)
├── models/          # Domain models
├── scanner/         # Music library scanner
└── crypto/          # Password hashing
```

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

- [Subsonic](http://www.subsonic.org/) - Original API specification
- [OpenSubsonic](https://opensubsonic.netlify.app/) - Modern API extensions
- [Navidrome](https://www.navidrome.org/) - Inspiration for implementation
