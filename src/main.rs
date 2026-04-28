//! Subsonic API compatible server.

use std::path::PathBuf;

use axum::{Router, extract::FromRef};
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
use clap::{Parser, Subcommand};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use suboxide::api::auth::AuthStateHandle;
use suboxide::api::{MusicService, RemoteControlService, SubsonicRouterExt, UserService, handlers};
use suboxide::crypto::{PasswordError, hash_password};
use suboxide::db::{
    DbConfig, DbPool, MusicFolderRepository, MusicRepoError, NewUser, UserRepoError,
    UserRepository, UserUpdate, run_migrations,
};
use suboxide::lastfm::{LastFmClient, LastFmError};
use suboxide::models::music::NewMusicFolder;
use suboxide::scanner::{AutoScanner, ScanError, ScanMode, ScanState, ScanStateHandle, Scanner};

/// Subsonic-compatible music streaming server.
#[derive(Parser)]
#[command(name = "suboxide")]
#[command(about = "A Subsonic API compatible music server written in Rust")]
struct Cli {
    /// Database file path
    #[arg(short, long, default_value = "suboxide.db")]
    database: PathBuf,

    /// Server port
    #[arg(short, long, default_value = "4040")]
    port: u16,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage users
    #[command(subcommand)]
    User(UserCommands),

    /// Manage API keys
    #[command(subcommand)]
    ApiKey(ApiKeyCommands),

    /// Manage music folders
    #[command(subcommand)]
    Folder(FolderCommands),

    /// Manage Last.fm integration
    #[command(subcommand)]
    Lastfm(LastfmCommands),

    /// Scan music folders for audio files
    Scan {
        /// Specific folder ID to scan (scans all if not specified)
        #[arg(short, long)]
        folder: Option<i32>,

        /// Run full scan (re-scan all files regardless of modification time)
        #[arg(long)]
        full: bool,
    },

    /// Start the server (default)
    Serve {
        /// Enable auto-scan (periodic incremental scanning)
        #[arg(long)]
        auto_scan: bool,

        /// Auto-scan interval in seconds (default: 300 = 5 minutes)
        #[arg(long, default_value = "300")]
        auto_scan_interval: u64,
    },
}

#[derive(Subcommand)]
enum UserCommands {
    /// Create a new user
    Create {
        /// Username
        #[arg(short, long)]
        username: String,

        /// Password
        #[arg(short, long)]
        password: String,

        /// Create as admin user
        #[arg(short, long)]
        admin: bool,
    },

    /// List all users
    List,

    /// Update a user
    Update {
        /// Username of the user to update
        #[arg(short, long)]
        username: String,

        /// New password
        #[arg(short, long)]
        password: Option<String>,

        /// Set admin role
        #[arg(long)]
        admin: Option<bool>,

        /// Set email
        #[arg(long)]
        email: Option<String>,
    },

    /// Delete a user
    Delete {
        /// Username of the user to delete
        #[arg(short, long)]
        username: String,
    },
}

#[derive(Subcommand)]
enum ApiKeyCommands {
    /// Generate an API key for a user
    Generate {
        /// Username of the user
        #[arg(short, long)]
        username: String,
    },

    /// Revoke (delete) an API key for a user
    Revoke {
        /// Username of the user
        #[arg(short, long)]
        username: String,
    },

    /// Show a user's API key
    Show {
        /// Username of the user
        #[arg(short, long)]
        username: String,
    },
}

#[derive(Subcommand)]
enum FolderCommands {
    /// Add a music folder
    Add {
        /// Name of the music folder
        #[arg(short, long)]
        name: String,

        /// Path to the music folder
        #[arg(short, long)]
        path: PathBuf,
    },

    /// List all music folders
    List,

    /// Remove a music folder
    Remove {
        /// ID of the folder to remove
        #[arg(short, long)]
        id: i32,
    },
}

#[derive(Subcommand)]
enum LastfmCommands {
    /// Set a user's Last.fm session key manually
    Set {
        /// Username of the user
        #[arg(short, long)]
        username: String,

        /// Last.fm session key
        #[arg(short, long)]
        session_key: String,
    },

    /// Unlink (clear) a user's Last.fm session key
    Unlink {
        /// Username of the user
        #[arg(short, long)]
        username: String,
    },

    /// Interactively link a user's Last.fm account
    Link {
        /// Username of the user
        #[arg(short, long)]
        username: String,

        /// Token from Last.fm (optional, to bypass interactive prompt)
        #[arg(long)]
        token: Option<String>,
    },

    /// Debug Last.fm artist lookup
    Debug {
        /// Artist name to look up
        #[arg(short, long)]
        artist: String,
    },
}

/// Application state shared across all handlers.
#[derive(Clone, Debug)]
pub struct AppState {
    pool: DbPool,
    scan_state: ScanStateHandle,
    music: MusicService,
    users: UserService,
    remote: RemoteControlService,
}

impl AppState {
    /// Create a new application state with the given database pool and Last.fm client.
    #[must_use]
    pub fn new(pool: DbPool, lastfm_client: Option<LastFmClient>) -> Self {
        let scan_state = ScanStateHandle::new(ScanState::new());
        let music = MusicService::new(pool.clone(), lastfm_client);
        let users = UserService::new(pool.clone());
        let remote = RemoteControlService::new(pool.clone());

        Self {
            pool,
            scan_state,
            music,
            users,
            remote,
        }
    }

    /// Get the shared scan state for use by `AutoScanner`.
    #[must_use]
    pub fn scan_state(&self) -> ScanStateHandle {
        self.scan_state.clone()
    }
}

impl FromRef<AppState> for AuthStateHandle {
    fn from_ref(state: &AppState) -> Self {
        Self::new(state.users.clone())
    }
}

impl FromRef<AppState> for MusicService {
    fn from_ref(state: &AppState) -> Self {
        state.music.clone()
    }
}

impl FromRef<AppState> for UserService {
    fn from_ref(state: &AppState) -> Self {
        state.users.clone()
    }
}

impl FromRef<AppState> for RemoteControlService {
    fn from_ref(state: &AppState) -> Self {
        state.remote.clone()
    }
}

impl FromRef<AppState> for DbPool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for ScanStateHandle {
    fn from_ref(state: &AppState) -> Self {
        state.scan_state.clone()
    }
}

/// Create the main router with all Subsonic API routes.
/// All endpoints support both GET and POST with query-string parameters.
/// The .view suffix is automatically handled by `SubsonicRouterExt`.
fn create_router(state: AppState) -> Router {
    // All endpoints - subsonic_route automatically adds .view suffix and POST method.
    let rest_routes = Router::new()
        // System endpoints
        .subsonic_route("/ping", handlers::ping)
        .subsonic_route("/getLicense", handlers::get_license)
        .subsonic_route(
            "/getOpenSubsonicExtensions",
            handlers::get_open_subsonic_extensions,
        )
        .subsonic_route("/tokenInfo", handlers::token_info)
        // Bookmarks endpoints
        .subsonic_route("/getBookmarks", handlers::get_bookmarks)
        // Browsing endpoints
        .subsonic_route("/getMusicFolders", handlers::get_music_folders)
        .subsonic_route("/getIndexes", handlers::get_indexes)
        .subsonic_route("/getArtists", handlers::get_artists)
        .subsonic_route("/getArtist", handlers::get_artist)
        .subsonic_route("/getAlbum", handlers::get_album)
        .subsonic_route("/getSong", handlers::get_song)
        .subsonic_route("/getAlbumList2", handlers::get_album_list2)
        .subsonic_route("/getGenres", handlers::get_genres)
        .subsonic_route("/search3", handlers::search3)
        .subsonic_route("/getRandomSongs", handlers::get_random_songs)
        .subsonic_route("/getSongsByGenre", handlers::get_songs_by_genre)
        .subsonic_route("/getArtistInfo2", handlers::get_artist_info2)
        .subsonic_route("/getAlbumInfo2", handlers::get_album_info2)
        .subsonic_route("/getSimilarSongs2", handlers::get_similar_songs2)
        .subsonic_route("/getTopSongs", handlers::get_top_songs)
        // Non-ID3 browsing endpoints (for older clients)
        .subsonic_route("/getMusicDirectory", handlers::get_music_directory)
        .subsonic_route("/getAlbumList", handlers::get_album_list)
        .subsonic_route("/getStarred", handlers::get_starred)
        .subsonic_route("/getArtistInfo", handlers::get_artist_info)
        .subsonic_route("/getAlbumInfo", handlers::get_album_info)
        .subsonic_route("/getSimilarSongs", handlers::get_similar_songs)
        // Search endpoints
        .subsonic_route("/search2", handlers::search2)
        .subsonic_route("/search", handlers::search)
        // Lyrics endpoints
        .subsonic_route("/getLyrics", handlers::get_lyrics)
        .subsonic_route("/getLyricsBySongId", handlers::get_lyrics_by_song_id)
        // Annotation endpoints
        .subsonic_route("/star", handlers::star)
        .subsonic_route("/unstar", handlers::unstar)
        .subsonic_route("/getStarred2", handlers::get_starred2)
        .subsonic_route("/scrobble", handlers::scrobble)
        .subsonic_route("/getNowPlaying", handlers::get_now_playing)
        .subsonic_route("/setRating", handlers::set_rating)
        // Playlist endpoints
        .subsonic_route("/getPlaylists", handlers::get_playlists)
        .subsonic_route("/getPlaylist", handlers::get_playlist)
        .subsonic_route("/createPlaylist", handlers::create_playlist)
        .subsonic_route("/updatePlaylist", handlers::update_playlist)
        .subsonic_route("/deletePlaylist", handlers::delete_playlist)
        // Play queue endpoints
        .subsonic_route("/getPlayQueue", handlers::get_play_queue)
        .subsonic_route("/savePlayQueue", handlers::save_play_queue)
        // Play queue by index endpoints (OpenSubsonic extension)
        .subsonic_route("/getPlayQueueByIndex", handlers::get_play_queue_by_index)
        .subsonic_route("/savePlayQueueByIndex", handlers::save_play_queue_by_index)
        // Remote control endpoints (OpenSubsonic extension)
        .subsonic_route("/createRemoteSession", handlers::create_remote_session)
        .subsonic_route("/joinRemoteSession", handlers::join_remote_session)
        .subsonic_route("/getRemoteSession", handlers::get_remote_session)
        .subsonic_route("/closeRemoteSession", handlers::close_remote_session)
        .subsonic_route("/sendRemoteCommand", handlers::send_remote_command)
        .subsonic_route("/getRemoteCommands", handlers::get_remote_commands)
        .subsonic_route("/updateRemoteState", handlers::update_remote_state)
        .subsonic_route("/getRemoteState", handlers::get_remote_state)
        // Media retrieval endpoints
        .subsonic_route("/stream", handlers::stream)
        .subsonic_route("/download", handlers::download)
        .subsonic_route("/getCoverArt", handlers::get_cover_art)
        // User management endpoints
        .subsonic_route("/getUser", handlers::get_user)
        .subsonic_route("/getUsers", handlers::get_users)
        .subsonic_route("/deleteUser", handlers::delete_user)
        .subsonic_route("/changePassword", handlers::change_password)
        .subsonic_route("/createUser", handlers::create_user)
        .subsonic_route("/updateUser", handlers::update_user)
        // Scanning endpoints
        .subsonic_route("/startScan", handlers::start_scan)
        .subsonic_route("/getScanStatus", handlers::get_scan_status);

    Router::new()
        .nest("/rest", rest_routes)
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Errors that can occur during database setup.
#[derive(Debug, thiserror::Error)]
enum SetupError {
    #[error("Failed to create database pool: {0}")]
    PoolCreation(String),
    #[error("Failed to get database connection: {0}")]
    Connection(String),
    #[error("Failed to run migrations: {0}")]
    Migration(String),
}

fn setup_database(database_path: impl AsRef<std::path::Path>) -> Result<DbPool, SetupError> {
    let database_url = database_path
        .as_ref()
        .to_str()
        .ok_or_else(|| SetupError::PoolCreation("Invalid UTF-8 in database path".to_string()))?;
    let config = DbConfig::new(database_url);
    let pool = config
        .build_pool()
        .map_err(|e| SetupError::PoolCreation(e.to_string()))?;

    // Run migrations
    let mut conn = pool
        .get()
        .map_err(|e| SetupError::Connection(e.to_string()))?;
    run_migrations(&mut conn).map_err(|e| SetupError::Migration(e.to_string()))?;

    Ok(pool)
}

fn create_user(
    pool: &DbPool,
    username: &str,
    password: &str,
    admin: bool,
) -> Result<(), UserCommandError> {
    let password_hash = hash_password(password)?;
    let repo = UserRepository::new(pool.clone());

    let new_user = if admin {
        NewUser::admin(username, &password_hash, password)
    } else {
        NewUser::regular(username, &password_hash, password)
    };

    match repo.create(&new_user) {
        Ok(user) => {
            println!(
                "Created user '{}' (id: {}, admin: {})",
                user.username, user.id, admin
            );
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, thiserror::Error)]
enum UserCommandError {
    #[error("User '{0}' not found")]
    NotFound(String),
    #[error(transparent)]
    Password(#[from] PasswordError),
    #[error(transparent)]
    Repository(#[from] UserRepoError),
}

fn run_user_command(pool: &DbPool, cmd: UserCommands) -> Result<(), UserCommandError> {
    match cmd {
        UserCommands::Create {
            username,
            password,
            admin,
        } => {
            create_user(pool, &username, &password, admin)?;
        }
        UserCommands::List => {
            let users = UserService::new(pool.clone());
            let users = users.get_all_users()?;
            if users.is_empty() {
                println!("No users found.");
            } else {
                println!("Users:");
                for user in users {
                    let roles: Vec<&str> = [
                        (user.roles.admin_role, "admin"),
                        (user.roles.settings_role, "settings"),
                        (user.roles.stream_role, "stream"),
                        (user.roles.download_role, "download"),
                        (user.roles.upload_role, "upload"),
                        (user.roles.jukebox_role, "jukebox"),
                        (user.roles.playlist_role, "playlist"),
                        (user.roles.cover_art_role, "cover_art"),
                        (user.roles.comment_role, "comment"),
                        (user.roles.podcast_role, "podcast"),
                        (user.roles.share_role, "share"),
                        (user.roles.video_conversion_role, "video"),
                    ]
                    .iter()
                    .filter(|(enabled, _)| *enabled)
                    .map(|(_, name)| *name)
                    .collect();

                    println!(
                        "  [{}] {} (roles: {})",
                        user.id,
                        user.username,
                        roles.join(", ")
                    );
                }
            }
        }
        UserCommands::Update {
            username,
            password,
            admin,
            email,
        } => {
            let repo = UserRepository::new(pool.clone());
            let mut builder = UserUpdate::builder(&username);

            if let Some(email) = email {
                builder = builder.email(email);
            }
            if let Some(admin) = admin {
                builder = builder.admin_role(admin);
            }

            let update = builder.build();

            let user_id = match repo.find_by_username(&username)? {
                Some(user) => user.id,
                None => return Err(UserCommandError::NotFound(username.clone())),
            };

            if repo.update_user(&update)? {
                println!("Updated user details for '{username}'");
            } else {
                return Err(UserCommandError::NotFound(username));
            }

            if let Some(password) = password {
                let hash = hash_password(&password)?;
                repo.update_password(user_id, &hash)?;
                println!("Updated password for '{username}'");
            }
        }
        UserCommands::Delete { username } => {
            let users = UserService::new(pool.clone());
            if users.delete_user(&username)? {
                println!("Deleted user '{username}'");
            } else {
                return Err(UserCommandError::NotFound(username));
            }
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum ApiKeyCommandError {
    #[error("User '{0}' not found")]
    NotFound(String),
    #[error(transparent)]
    Repository(#[from] UserRepoError),
}

fn run_api_key_command(pool: &DbPool, cmd: ApiKeyCommands) -> Result<(), ApiKeyCommandError> {
    let repo = UserRepository::new(pool.clone());
    match cmd {
        ApiKeyCommands::Generate { username } => {
            let Some(user) = repo.find_by_username(&username)? else {
                return Err(ApiKeyCommandError::NotFound(username));
            };
            let api_key = repo.generate_api_key(user.id)?;
            println!("Generated API key for user '{}':", user.username);
            println!("{api_key}");
        }
        ApiKeyCommands::Revoke { username } => {
            let Some(user) = repo.find_by_username(&username)? else {
                return Err(ApiKeyCommandError::NotFound(username));
            };
            if repo.revoke_api_key(user.id)? {
                println!("Revoked API key for user '{username}'");
            } else {
                return Err(ApiKeyCommandError::NotFound(username));
            }
        }
        ApiKeyCommands::Show { username } => {
            let Some(user) = repo.find_by_username(&username)? else {
                return Err(ApiKeyCommandError::NotFound(username));
            };
            if let Some(api_key) = user.api_key {
                println!("API key for user '{username}':");
                println!("{api_key}");
            } else {
                println!("User '{username}' has no API key. Generate one with:");
                println!("  suboxide api-key generate --username {username}");
            }
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum FolderCommandError {
    #[error("Music folder not found")]
    NotFound,
    #[error(transparent)]
    Repository(#[from] MusicRepoError),
}

fn run_folder_command(pool: &DbPool, cmd: FolderCommands) -> Result<(), FolderCommandError> {
    let repo = MusicFolderRepository::new(pool.clone());
    match cmd {
        FolderCommands::Add { name, path } => {
            let path_str = path.to_string_lossy().into_owned();
            let new_folder = NewMusicFolder::new(&name, &path_str);
            let folder = repo.create(&new_folder)?;
            println!("Added music folder '{}' (id: {})", folder.name, folder.id);
            println!("  Path: {}", folder.path);
        }
        FolderCommands::List => {
            let folders = repo.find_all()?;
            if folders.is_empty() {
                println!("No music folders configured. Add one with:");
                println!("  suboxide folder add --name \"Music\" --path /path/to/music");
            } else {
                println!("Music folders:");
                for folder in folders {
                    let status = if folder.enabled {
                        "enabled"
                    } else {
                        "disabled"
                    };
                    println!(
                        "  [{}] {} - {} ({})",
                        folder.id, folder.name, folder.path, status
                    );
                }
            }
        }
        FolderCommands::Remove { id } => {
            if repo.delete(id)? {
                println!("Removed music folder with id {id}");
            } else {
                return Err(FolderCommandError::NotFound);
            }
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum LastfmCommandError {
    #[error("User '{0}' not found")]
    UserNotFound(String),
    #[error(transparent)]
    UserRepo(#[from] UserRepoError),
    #[error(transparent)]
    LastFm(#[from] LastFmError),
    #[error(
        "Last.fm is not configured. Set LASTFM_API_KEY and LASTFM_API_SECRET environment variables."
    )]
    NotConfigured,
    #[error("Operation cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

async fn run_lastfm_command(pool: &DbPool, cmd: LastfmCommands) -> Result<(), LastfmCommandError> {
    let repo = UserRepository::new(pool.clone());
    match cmd {
        LastfmCommands::Set {
            username,
            session_key,
        } => {
            let Some(user) = repo.find_by_username(&username)? else {
                return Err(LastfmCommandError::UserNotFound(username));
            };
            if repo.set_lastfm_session_key(user.id, Some(&session_key))? {
                println!("Set Last.fm session key for user '{}'", user.username);
            } else {
                return Err(LastfmCommandError::UserNotFound(user.username));
            }
        }
        LastfmCommands::Unlink { username } => {
            let Some(user) = repo.find_by_username(&username)? else {
                return Err(LastfmCommandError::UserNotFound(username));
            };
            if repo.set_lastfm_session_key(user.id, None)? {
                println!("Cleared Last.fm session key for user '{username}'");
            } else {
                return Err(LastfmCommandError::UserNotFound(username));
            }
        }
        LastfmCommands::Link { username, token } => {
            let api_key = std::env::var("LASTFM_API_KEY").unwrap_or_default();
            let api_secret = std::env::var("LASTFM_API_SECRET").unwrap_or_default();

            let client = LastFmClient::new(api_key.clone(), api_secret)
                .ok_or(LastfmCommandError::NotConfigured)?;

            let Some(user) = repo.find_by_username(&username)? else {
                return Err(LastfmCommandError::UserNotFound(username.clone()));
            };

            let token = if let Some(t) = token {
                t
            } else {
                println!("To link your Last.fm account, please visit:");
                println!(
                    "http://www.last.fm/api/auth/?api_key={}&cb=http://localhost:8080/callback",
                    client.api_key()
                );
                println!(
                    "\nAfter approving access, you will be redirected to a URL (or see a token)."
                );
                println!("Please paste the 'token' parameter from the URL here:");

                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                let trimmed = input.trim();

                if trimmed.is_empty() {
                    return Err(LastfmCommandError::Cancelled);
                }
                trimmed.to_string()
            };

            println!("Exchanging token for session...");
            let session = client.get_session(&token).await?;
            println!(
                "Successfully authenticated as Last.fm user: {}",
                session.name
            );
            let _ = repo.set_lastfm_session_key(user.id, Some(&session.key))?;
            println!("Last.fm session linked successfully!");
        }
        LastfmCommands::Debug { artist } => {
            let api_key = std::env::var("LASTFM_API_KEY").unwrap_or_default();
            let api_secret = std::env::var("LASTFM_API_SECRET").unwrap_or_default();

            let client =
                LastFmClient::new(api_key, api_secret).ok_or(LastfmCommandError::NotConfigured)?;

            println!("Querying Last.fm for artist: '{artist}'");
            match client.get_artist_info(&artist).await? {
                Some(info) => {
                    println!("Found artist: {}", info.name);
                    println!("URL: {:?}", info.url);
                    println!("MBID: {:?}", info.musicbrainz_id);
                    println!("Images:");
                    for img in &info.image {
                        println!("  - [{}] {}", img.size, img.url);
                        if img.url.contains("2a96cbd8b46e442fc41c2b86b821562f") {
                            println!(
                                "    ^ WARNING: This hash is known to be a placeholder star image!"
                            );
                        }
                    }
                }
                None => println!("Artist not found on Last.fm"),
            }
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
enum ScanCommandError {
    #[error(transparent)]
    Scan(#[from] ScanError),
}

fn run_scan_command(
    pool: &DbPool,
    folder: Option<i32>,
    full: bool,
) -> Result<(), ScanCommandError> {
    let scanner = Scanner::new(pool.clone());
    let mode = if full {
        ScanMode::Full
    } else {
        ScanMode::Incremental
    };

    let result = folder.map_or_else(
        || scanner.scan_all_with_options(None, mode),
        |folder_id| scanner.scan_folder_by_id_with_mode(folder_id, mode),
    );

    let stats = result?;
    println!("\nScan complete:");
    println!("  Tracks found:     {}", stats.tracks_found);
    println!("  Tracks added:     {}", stats.tracks_added);
    println!("  Tracks updated:   {}", stats.tracks_updated);
    println!("  Tracks skipped:   {}", stats.tracks_skipped);
    println!("  Tracks removed:   {}", stats.tracks_removed);
    println!("  Tracks failed:    {}", stats.tracks_failed);
    println!("  Artists added:    {}", stats.artists_added);
    println!("  Albums added:     {}", stats.albums_added);
    println!("  Cover art saved:  {}", stats.cover_art_saved);
    Ok(())
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "suboxide=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Setup database
    let pool = match setup_database(&cli.database) {
        Ok(pool) => pool,
        Err(e) => {
            tracing::event!(
                name: "db.setup.failed",
                tracing::Level::ERROR,
                error = %e,
                "database setup failed"
            );
            std::process::exit(1);
        }
    };

    match cli.command {
        Some(Commands::User(cmd)) => {
            if let Err(e) = run_user_command(&pool, cmd) {
                tracing::error!(error = %e, "User command failed");
                std::process::exit(1);
            }
        }
        Some(Commands::ApiKey(cmd)) => {
            if let Err(e) = run_api_key_command(&pool, cmd) {
                tracing::error!(error = %e, "API key command failed");
                std::process::exit(1);
            }
        }
        Some(Commands::Folder(cmd)) => {
            if let Err(e) = run_folder_command(&pool, cmd) {
                tracing::error!(error = %e, "Folder command failed");
                std::process::exit(1);
            }
        }
        Some(Commands::Lastfm(cmd)) => {
            if let Err(e) = run_lastfm_command(&pool, cmd).await {
                tracing::error!(error = %e, "Last.fm command failed");
                std::process::exit(1);
            }
        }
        Some(Commands::Scan { folder, full }) => {
            if let Err(e) = run_scan_command(&pool, folder, full) {
                tracing::event!(
                    name: "scan.command.failed",
                    tracing::Level::ERROR,
                    error = %e,
                    "scan command failed"
                );
                std::process::exit(1);
            }
        }
        Some(Commands::Serve {
            auto_scan,
            auto_scan_interval,
        }) => {
            // Read Last.fm credentials before starting server
            let api_key = std::env::var("LASTFM_API_KEY").unwrap_or_default();
            let api_secret = std::env::var("LASTFM_API_SECRET").unwrap_or_default();
            let lastfm_client = LastFmClient::new(api_key, api_secret);

            if let Err(e) =
                run_server(pool, cli.port, auto_scan, auto_scan_interval, lastfm_client).await
            {
                tracing::event!(
                    name: "server.run.failed",
                    tracing::Level::ERROR,
                    error = %e,
                    "server failed"
                );
                std::process::exit(1);
            }
        }
        None => {
            // Default: start server without auto-scan
            let api_key = std::env::var("LASTFM_API_KEY").unwrap_or_default();
            let api_secret = std::env::var("LASTFM_API_SECRET").unwrap_or_default();
            let lastfm_client = LastFmClient::new(api_key, api_secret);

            if let Err(e) = run_server(pool, cli.port, false, 300, lastfm_client).await {
                tracing::event!(
                    name: "server.run.failed",
                    tracing::Level::ERROR,
                    error = %e,
                    "server failed"
                );
                std::process::exit(1);
            }
        }
    }
}

/// Errors that can occur during server startup.
#[derive(Debug, thiserror::Error)]
enum ServerError {
    #[error("Failed to check users: {0}")]
    UserCheck(#[source] UserRepoError),
    #[error("Failed to bind to address: {0}")]
    Bind(#[source] std::io::Error),
    #[error("Failed to get local address: {0}")]
    LocalAddr(#[source] std::io::Error),
    #[error("Server error: {0}")]
    Serve(#[source] std::io::Error),
}

async fn run_server(
    pool: DbPool,
    port: u16,
    auto_scan: bool,
    auto_scan_interval: u64,
    lastfm_client: Option<LastFmClient>,
) -> Result<(), ServerError> {
    // Check if there are any users
    let users = UserService::new(pool.clone());
    let has_users = users
        .get_all_users()
        .map_err(ServerError::UserCheck)?
        .is_empty();
    if !has_users {
        tracing::event!(
            name: "server.bootstrap.no_users",
            tracing::Level::WARN,
            "no users found in database"
        );
        tracing::event!(
            name: "server.bootstrap.user_create_hint",
            tracing::Level::WARN,
            command = "suboxide user create --username admin --password <password> --admin",
            "create initial user with command"
        );
    }

    let state = AppState::new(pool.clone(), lastfm_client);
    let app = create_router(state.clone());

    // Start auto-scanner if enabled, sharing the same scan state with the API
    let _auto_scan_handle = if auto_scan {
        let scan_state = state.scan_state();
        let auto_scanner = AutoScanner::with_interval(pool, scan_state, auto_scan_interval);
        tracing::event!(
            name: "scan.auto.enabled",
            tracing::Level::INFO,
            scan.interval_secs = auto_scan_interval,
            "auto-scan enabled"
        );
        Some(auto_scanner.start())
    } else {
        None
    };

    let addr = format!("0.0.0.0:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(ServerError::Bind)?;

    let local_addr = listener.local_addr().map_err(ServerError::LocalAddr)?;
    tracing::event!(
        name: "server.listen.started",
        tracing::Level::INFO,
        server.address = %local_addr,
        "suboxide server listening"
    );

    axum::serve(listener, app)
        .await
        .map_err(ServerError::Serve)?;

    Ok(())
}
