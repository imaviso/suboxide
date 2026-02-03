//! Subsonic API compatible server.

use std::path::PathBuf;
use std::sync::Arc;

use axum::{Router, extract::FromRef};
use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
use clap::{Parser, Subcommand};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use subsonic::api::{AuthState, DatabaseAuthState, SubsonicRouterExt, handlers};
use subsonic::crypto::hash_password;
use subsonic::db::{
    DbConfig, DbPool, MusicFolderRepository, NewUser, UserRepository, UserUpdate, run_migrations,
};
use subsonic::lastfm::LastFmClient;
use subsonic::models::music::NewMusicFolder;
use subsonic::scanner::{AutoScanner, ScanMode, ScanState, ScanStateHandle, Scanner};

/// Subsonic-compatible music streaming server.
#[derive(Parser)]
#[command(name = "subsonic")]
#[command(about = "A Subsonic API compatible music server written in Rust")]
struct Cli {
    /// Database file path
    #[arg(short, long, default_value = "subsonic.db")]
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
#[derive(Clone)]
pub struct AppState {
    auth: Arc<DatabaseAuthState>,
    scan_state: ScanStateHandle,
}

impl AppState {
    /// Create a new application state with the given database pool.
    #[must_use]
    pub fn new(pool: DbPool) -> Self {
        let scan_state = ScanStateHandle::new(ScanState::new());

        // Read Last.fm credentials from environment
        let api_key = std::env::var("LASTFM_API_KEY").unwrap_or_default();
        let api_secret = std::env::var("LASTFM_API_SECRET").unwrap_or_default();
        let lastfm_client = LastFmClient::new(api_key, api_secret);

        if lastfm_client.is_some() {
            tracing::info!("Last.fm integration enabled");
        } else {
            tracing::debug!(
                "Last.fm not configured (set LASTFM_API_KEY and LASTFM_API_SECRET to enable)"
            );
        }

        Self {
            auth: Arc::new(DatabaseAuthState::with_scan_state(
                pool,
                scan_state.clone(),
                lastfm_client,
            )),
            scan_state,
        }
    }

    /// Get the shared scan state for use by `AutoScanner`.
    #[must_use]
    pub fn scan_state(&self) -> ScanStateHandle {
        self.scan_state.clone()
    }
}

// Allow extracting Arc<dyn AuthState> from AppState
impl FromRef<AppState> for Arc<dyn AuthState> {
    fn from_ref(state: &AppState) -> Self {
        state.auth.clone()
    }
}

/// Create the main router with all Subsonic API routes.
/// All endpoints support both GET and POST (formPost extension).
/// The .view suffix is automatically handled by `SubsonicRouterExt`.
fn create_router(state: AppState) -> Router {
    // All endpoints - subsonic_route automatically adds .view suffix and POST method
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
#[derive(Debug)]
enum SetupError {
    PoolCreation(String),
    Connection(String),
    Migration(String),
}

impl std::fmt::Display for SetupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PoolCreation(msg) => write!(f, "Failed to create database pool: {msg}"),
            Self::Connection(msg) => write!(f, "Failed to get database connection: {msg}"),
            Self::Migration(msg) => write!(f, "Failed to run migrations: {msg}"),
        }
    }
}

impl std::error::Error for SetupError {}

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
) -> Result<(), Box<dyn std::error::Error>> {
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
        Err(e) => {
            eprintln!("Failed to create user: {e}");
            Err(Box::new(e))
        }
    }
}

#[tokio::main]
#[expect(clippy::too_many_lines)]
async fn main() {
    let cli = Cli::parse();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "subsonic=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Setup database
    let pool = match setup_database(&cli.database) {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!(error = %e, "Database setup failed");
            std::process::exit(1);
        }
    };

    match cli.command {
        Some(Commands::User(user_command)) => match user_command {
            UserCommands::Create {
                username,
                password,
                admin,
            } => {
                if create_user(&pool, &username, &password, admin).is_err() {
                    std::process::exit(1);
                }
            }
            UserCommands::List => {
                let repo = UserRepository::new(pool);
                match repo.find_all() {
                    Ok(users) => {
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
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to list users");
                        std::process::exit(1);
                    }
                }
            }
            UserCommands::Update {
                username,
                password,
                admin,
                email,
            } => {
                let repo = UserRepository::new(pool);
                let mut builder = UserUpdate::builder(&username);

                if let Some(email) = email {
                    builder = builder.email(email);
                }
                if let Some(admin) = admin {
                    builder = builder.admin_role(admin);
                }

                let update = builder.build();

                // Check if user exists first to get ID for password update
                let user_id = match repo.find_by_username(&username) {
                    Ok(Some(user)) => user.id,
                    Ok(None) => {
                        tracing::error!(username = %username, "User not found");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, username = %username, "Database error");
                        std::process::exit(1);
                    }
                };

                // Update fields
                match repo.update_user(&update) {
                    Ok(true) => println!("Updated user details for '{username}'"),
                    Ok(false) => {
                        // Should be caught by find_by_username, but just in case
                        tracing::error!(username = %username, "User not found during update");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, username = %username, "Failed to update user");
                        std::process::exit(1);
                    }
                }

                // Update password if provided
                if let Some(password) = password {
                    match hash_password(&password) {
                        Ok(hash) => match repo.update_password(user_id, &hash) {
                            Ok(true) => println!("Updated password for '{username}'"),
                            Ok(false) => {
                                tracing::error!("Failed to update password (user missing?)");
                            }
                            Err(e) => {
                                tracing::error!(error = %e, "Failed to update password in DB");
                            }
                        },
                        Err(e) => tracing::error!(error = %e, "Failed to hash password"),
                    }
                }
            }
            UserCommands::Delete { username } => {
                let repo = UserRepository::new(pool);
                // Need to find ID first because delete takes ID
                match repo.find_by_username(&username) {
                    Ok(Some(user)) => match repo.delete(user.id) {
                        Ok(true) => println!("Deleted user '{username}'"),
                        Ok(false) => {
                            tracing::error!(username = %username, "User not found when deleting");
                            std::process::exit(1);
                        }
                        Err(e) => {
                            tracing::error!(error = %e, username = %username, "Failed to delete user");
                            std::process::exit(1);
                        }
                    },
                    Ok(None) => {
                        tracing::error!(username = %username, "User not found");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, username = %username, "Database error");
                        std::process::exit(1);
                    }
                }
            }
        },
        Some(Commands::ApiKey(api_key_command)) => match api_key_command {
            ApiKeyCommands::Generate { username } => {
                let repo = UserRepository::new(pool);
                match repo.find_by_username(&username) {
                    Ok(Some(user)) => match repo.generate_api_key(user.id) {
                        Ok(api_key) => {
                            println!("Generated API key for user '{username}':");
                            println!("{api_key}");
                        }
                        Err(e) => {
                            tracing::error!(error = %e, username = %username, "Failed to generate API key");
                            std::process::exit(1);
                        }
                    },
                    Ok(None) => {
                        tracing::error!(username = %username, "User not found");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, username = %username, "Database error while finding user");
                        std::process::exit(1);
                    }
                }
            }
            ApiKeyCommands::Revoke { username } => {
                let repo = UserRepository::new(pool);
                match repo.find_by_username(&username) {
                    Ok(Some(user)) => match repo.revoke_api_key(user.id) {
                        Ok(true) => {
                            println!("Revoked API key for user '{username}'");
                        }
                        Ok(false) => {
                            tracing::error!(username = %username, "User not found when revoking API key");
                            std::process::exit(1);
                        }
                        Err(e) => {
                            tracing::error!(error = %e, username = %username, "Failed to revoke API key");
                            std::process::exit(1);
                        }
                    },
                    Ok(None) => {
                        tracing::error!(username = %username, "User not found");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, username = %username, "Database error");
                        std::process::exit(1);
                    }
                }
            }
            ApiKeyCommands::Show { username } => {
                let repo = UserRepository::new(pool);
                match repo.find_by_username(&username) {
                    Ok(Some(user)) => {
                        if let Some(api_key) = user.api_key {
                            println!("API key for user '{username}':");
                            println!("{api_key}");
                        } else {
                            println!("User '{username}' has no API key. Generate one with:");
                            println!("  subsonic api-key generate --username {username}");
                        }
                    }
                    Ok(None) => {
                        tracing::error!(username = %username, "User not found");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, username = %username, "Database error");
                        std::process::exit(1);
                    }
                }
            }
        },
        Some(Commands::Folder(folder_command)) => match folder_command {
            FolderCommands::Add { name, path } => {
                let path_str = path.to_string_lossy().into_owned();
                let repo = MusicFolderRepository::new(pool);
                let new_folder = NewMusicFolder::new(&name, &path_str);
                match repo.create(&new_folder) {
                    Ok(folder) => {
                        println!("Added music folder '{}' (id: {})", folder.name, folder.id);
                        println!("  Path: {}", folder.path);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, folder_name = %name, "Failed to add music folder");
                        std::process::exit(1);
                    }
                }
            }
            FolderCommands::List => {
                let repo = MusicFolderRepository::new(pool);
                match repo.find_all() {
                    Ok(folders) => {
                        if folders.is_empty() {
                            println!("No music folders configured. Add one with:");
                            println!(
                                "  subsonic folder add --name \"Music\" --path /path/to/music"
                            );
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
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to list music folders");
                        std::process::exit(1);
                    }
                }
            }
            FolderCommands::Remove { id } => {
                let repo = MusicFolderRepository::new(pool);
                match repo.delete(id) {
                    Ok(true) => {
                        println!("Removed music folder with id {id}");
                    }
                    Ok(false) => {
                        tracing::error!(folder_id = id, "Music folder not found");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, folder_id = id, "Failed to remove music folder");
                        std::process::exit(1);
                    }
                }
            }
        },
        Some(Commands::Lastfm(lastfm_command)) => match lastfm_command {
            LastfmCommands::Set {
                username,
                session_key,
            } => {
                let repo = UserRepository::new(pool);
                match repo.find_by_username(&username) {
                    Ok(Some(user)) => {
                        match repo.set_lastfm_session_key(user.id, Some(&session_key)) {
                            Ok(true) => {
                                println!("Set Last.fm session key for user '{username}'");
                            }
                            Ok(false) => {
                                tracing::error!(
                                    username = %username,
                                    "User not found when setting Last.fm key"
                                );
                                std::process::exit(1);
                            }
                            Err(e) => {
                                tracing::error!(
                                    error = %e,
                                    username = %username,
                                    "Failed to set Last.fm session key"
                                );
                                std::process::exit(1);
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::error!(username = %username, "User not found");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, username = %username, "Database error");
                        std::process::exit(1);
                    }
                }
            }
            LastfmCommands::Unlink { username } => {
                let repo = UserRepository::new(pool);
                match repo.find_by_username(&username) {
                    Ok(Some(user)) => match repo.set_lastfm_session_key(user.id, None) {
                        Ok(true) => {
                            println!("Cleared Last.fm session key for user '{username}'");
                        }
                        Ok(false) => {
                            tracing::error!(
                                username = %username,
                                "User not found when clearing Last.fm key"
                            );
                            std::process::exit(1);
                        }
                        Err(e) => {
                            tracing::error!(
                                error = %e,
                                username = %username,
                                "Failed to clear Last.fm session key"
                            );
                            std::process::exit(1);
                        }
                    },
                    Ok(None) => {
                        tracing::error!(username = %username, "User not found");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        tracing::error!(error = %e, username = %username, "Database error");
                        std::process::exit(1);
                    }
                }
            }
            LastfmCommands::Link { username, token } => {
                // Check if Last.fm is configured
                let api_key = std::env::var("LASTFM_API_KEY").unwrap_or_default();
                let api_secret = std::env::var("LASTFM_API_SECRET").unwrap_or_default();

                let Some(client) = LastFmClient::new(api_key.clone(), api_secret) else {
                    eprintln!("Error: Last.fm integration is not configured.");
                    eprintln!(
                        "Please set LASTFM_API_KEY and LASTFM_API_SECRET environment variables."
                    );
                    std::process::exit(1);
                };

                // Find user first
                let repo = UserRepository::new(pool);
                let user = match repo.find_by_username(&username) {
                    Ok(Some(user)) => user,
                    Ok(None) => {
                        eprintln!("Error: User '{username}' not found.");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Error finding user: {e}");
                        std::process::exit(1);
                    }
                };

                let token = token.unwrap_or_else(|| {
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
                    if std::io::stdin().read_line(&mut input).is_err() {
                        eprintln!("Error reading input");
                        std::process::exit(1);
                    }
                    let trimmed = input.trim();

                    if trimmed.is_empty() {
                        eprintln!("Operation cancelled.");
                        std::process::exit(1);
                    }
                    trimmed.to_string()
                });

                println!("Exchanging token for session...");
                match client.get_session(&token).await {
                    Ok(session) => {
                        println!(
                            "Successfully authenticated as Last.fm user: {}",
                            session.name
                        );
                        match repo.set_lastfm_session_key(user.id, Some(&session.key)) {
                            Ok(_) => println!("Last.fm session linked successfully!"),
                            Err(e) => eprintln!("Failed to save session key: {e}"),
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to get session from Last.fm: {e}");
                        std::process::exit(1);
                    }
                }
            }
            LastfmCommands::Debug { artist } => {
                let api_key = std::env::var("LASTFM_API_KEY").unwrap_or_default();
                let api_secret = std::env::var("LASTFM_API_SECRET").unwrap_or_default();

                let Some(client) = LastFmClient::new(api_key, api_secret) else {
                    eprintln!("Error: Last.fm integration is not configured.");
                    eprintln!(
                        "Please set LASTFM_API_KEY and LASTFM_API_SECRET environment variables."
                    );
                    std::process::exit(1);
                };

                println!("Querying Last.fm for artist: '{artist}'");
                match client.get_artist_info(&artist).await {
                    Ok(Some(info)) => {
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
                    Ok(None) => {
                        println!("Artist not found on Last.fm");
                    }
                    Err(e) => {
                        eprintln!("Error querying Last.fm: {e}");
                        std::process::exit(1);
                    }
                }
            }
        },
        Some(Commands::Scan { folder, full }) => {
            let scanner = Scanner::new(pool);
            let mode = if full {
                ScanMode::Full
            } else {
                ScanMode::Incremental
            };

            let result = folder.map_or_else(
                || scanner.scan_all_with_options(None, mode),
                |folder_id| scanner.scan_folder_by_id_with_mode(folder_id, mode),
            );

            match result {
                Ok(stats) => {
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
                }
                Err(e) => {
                    tracing::error!(error = %e, "Scan failed");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Serve {
            auto_scan,
            auto_scan_interval,
        }) => {
            if let Err(e) = run_server(pool, cli.port, auto_scan, auto_scan_interval).await {
                tracing::error!(error = %e, "Server failed");
                std::process::exit(1);
            }
        }
        None => {
            // Default: start server without auto-scan
            if let Err(e) = run_server(pool, cli.port, false, 300).await {
                tracing::error!(error = %e, "Server failed");
                std::process::exit(1);
            }
        }
    }
}

/// Errors that can occur during server startup.
#[derive(Debug)]
enum ServerError {
    UserCheck(Box<dyn std::error::Error + Send + Sync + 'static>),
    Bind(std::io::Error),
    LocalAddr(std::io::Error),
    Serve(std::io::Error),
}

impl std::fmt::Display for ServerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UserCheck(e) => write!(f, "Failed to check users: {e}"),
            Self::Bind(e) => write!(f, "Failed to bind to address: {e}"),
            Self::LocalAddr(e) => write!(f, "Failed to get local address: {e}"),
            Self::Serve(e) => write!(f, "Server error: {e}"),
        }
    }
}

impl std::error::Error for ServerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::UserCheck(e) => Some(e.as_ref()),
            Self::Bind(e) | Self::LocalAddr(e) | Self::Serve(e) => Some(e),
        }
    }
}

async fn run_server(
    pool: DbPool,
    port: u16,
    auto_scan: bool,
    auto_scan_interval: u64,
) -> Result<(), ServerError> {
    // Check if there are any users
    let repo = UserRepository::new(pool.clone());
    let has_users = repo
        .has_users()
        .map_err(|e| ServerError::UserCheck(Box::new(e)))?;
    if !has_users {
        tracing::warn!("No users found in database");
        tracing::warn!(
            command = "subsonic user create --username admin --password <password> --admin",
            "Create a user with"
        );
    }

    let state = AppState::new(pool.clone());
    let app = create_router(state.clone());

    // Start auto-scanner if enabled, sharing the same scan state with the API
    let _auto_scan_handle = if auto_scan {
        let scan_state = state.scan_state();
        let mut auto_scanner = AutoScanner::with_interval(pool, scan_state, auto_scan_interval);
        tracing::info!(
            auto_scan_interval_secs = auto_scan_interval,
            "Auto-scan enabled"
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
    tracing::info!(addr = %local_addr, "Subsonic server listening");

    axum::serve(listener, app)
        .await
        .map_err(ServerError::Serve)?;

    Ok(())
}
