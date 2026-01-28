//! Subsonic API compatible server.

use std::sync::Arc;

use axum::{Router, extract::FromRef};
use clap::{Parser, Subcommand};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use subsonic::api::{AuthState, DatabaseAuthState, SubsonicRouterExt, handlers};
use subsonic::crypto::hash_password;
use subsonic::db::{
    DbConfig, DbPool, MusicFolderRepository, NewUser, UserRepository, run_migrations,
};
use subsonic::models::music::NewMusicFolder;
use subsonic::scanner::{AutoScanner, ScanMode, ScanState, Scanner};

/// Subsonic-compatible music streaming server.
#[derive(Parser)]
#[command(name = "subsonic")]
#[command(about = "A Subsonic API compatible music server written in Rust")]
struct Cli {
    /// Database file path
    #[arg(short, long, default_value = "subsonic.db")]
    database: String,

    /// Server port
    #[arg(short, long, default_value = "4040")]
    port: u16,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new user
    CreateUser {
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

    /// Generate an API key for a user
    GenerateApiKey {
        /// Username of the user to generate API key for
        #[arg(short, long)]
        username: String,
    },

    /// Revoke (delete) an API key for a user
    RevokeApiKey {
        /// Username of the user to revoke API key for
        #[arg(short, long)]
        username: String,
    },

    /// Show a user's API key
    ShowApiKey {
        /// Username of the user
        #[arg(short, long)]
        username: String,
    },

    /// Add a music folder
    AddFolder {
        /// Name of the music folder
        #[arg(short, long)]
        name: String,

        /// Path to the music folder
        #[arg(short, long)]
        path: String,
    },

    /// List all music folders
    ListFolders,

    /// Remove a music folder
    RemoveFolder {
        /// ID of the folder to remove
        #[arg(short, long)]
        id: i32,
    },

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

/// Application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    auth: Arc<DatabaseAuthState>,
    scan_state: Arc<ScanState>,
}

impl AppState {
    #[must_use]
    pub fn new(pool: DbPool) -> Self {
        let scan_state = Arc::new(ScanState::new());
        Self {
            auth: Arc::new(DatabaseAuthState::with_scan_state(pool, scan_state.clone())),
            scan_state,
        }
    }

    /// Get the shared scan state for use by `AutoScanner`.
    #[must_use]
    pub fn scan_state(&self) -> Arc<ScanState> {
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

fn setup_database(database_url: &str) -> DbPool {
    let config = DbConfig::new(database_url);
    let pool = config.build_pool().expect("Failed to create database pool");

    // Run migrations
    let mut conn = pool.get().expect("Failed to get database connection");
    run_migrations(&mut conn).expect("Failed to run migrations");

    pool
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
#[allow(clippy::too_many_lines)]
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
    let pool = setup_database(&cli.database);

    match cli.command {
        Some(Commands::CreateUser {
            username,
            password,
            admin,
        }) => {
            if create_user(&pool, &username, &password, admin).is_err() {
                std::process::exit(1);
            }
        }
        Some(Commands::GenerateApiKey { username }) => {
            let repo = UserRepository::new(pool);
            match repo.find_by_username(&username) {
                Ok(Some(user)) => match repo.generate_api_key(user.id) {
                    Ok(api_key) => {
                        println!("Generated API key for user '{username}':");
                        println!("{api_key}");
                    }
                    Err(e) => {
                        eprintln!("Failed to generate API key: {e}");
                        std::process::exit(1);
                    }
                },
                Ok(None) => {
                    eprintln!("User '{username}' not found");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Database error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::RevokeApiKey { username }) => {
            let repo = UserRepository::new(pool);
            match repo.find_by_username(&username) {
                Ok(Some(user)) => match repo.revoke_api_key(user.id) {
                    Ok(true) => {
                        println!("Revoked API key for user '{username}'");
                    }
                    Ok(false) => {
                        eprintln!("User '{username}' not found");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("Failed to revoke API key: {e}");
                        std::process::exit(1);
                    }
                },
                Ok(None) => {
                    eprintln!("User '{username}' not found");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Database error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::ShowApiKey { username }) => {
            let repo = UserRepository::new(pool);
            match repo.find_by_username(&username) {
                Ok(Some(user)) => {
                    if let Some(api_key) = user.api_key {
                        println!("API key for user '{username}':");
                        println!("{api_key}");
                    } else {
                        println!("User '{username}' has no API key. Generate one with:");
                        println!("  subsonic generate-api-key --username {username}");
                    }
                }
                Ok(None) => {
                    eprintln!("User '{username}' not found");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Database error: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::AddFolder { name, path }) => {
            let repo = MusicFolderRepository::new(pool);
            let new_folder = NewMusicFolder::new(&name, &path);
            match repo.create(&new_folder) {
                Ok(folder) => {
                    println!("Added music folder '{}' (id: {})", folder.name, folder.id);
                    println!("  Path: {}", folder.path);
                }
                Err(e) => {
                    eprintln!("Failed to add music folder: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::ListFolders) => {
            let repo = MusicFolderRepository::new(pool);
            match repo.find_all() {
                Ok(folders) => {
                    if folders.is_empty() {
                        println!("No music folders configured. Add one with:");
                        println!("  subsonic add-folder --name \"Music\" --path /path/to/music");
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
                    eprintln!("Failed to list music folders: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::RemoveFolder { id }) => {
            let repo = MusicFolderRepository::new(pool);
            match repo.delete(id) {
                Ok(true) => {
                    println!("Removed music folder with id {id}");
                }
                Ok(false) => {
                    eprintln!("Music folder with id {id} not found");
                    std::process::exit(1);
                }
                Err(e) => {
                    eprintln!("Failed to remove music folder: {e}");
                    std::process::exit(1);
                }
            }
        }
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
                    eprintln!("Scan failed: {e}");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Serve {
            auto_scan,
            auto_scan_interval,
        }) => {
            run_server(pool, cli.port, auto_scan, auto_scan_interval).await;
        }
        None => {
            // Default: start server without auto-scan
            run_server(pool, cli.port, false, 300).await;
        }
    }
}

async fn run_server(pool: DbPool, port: u16, auto_scan: bool, auto_scan_interval: u64) {
    // Check if there are any users
    let repo = UserRepository::new(pool.clone());
    if !repo.has_users().unwrap_or(false) {
        tracing::warn!("No users found in database. Create one with:");
        tracing::warn!("  subsonic create-user --username admin --password <password> --admin");
    }

    let state = AppState::new(pool.clone());
    let app = create_router(state.clone());

    // Start auto-scanner if enabled, sharing the same scan state with the API
    let _auto_scan_handle = if auto_scan {
        let scan_state = state.scan_state();
        let mut auto_scanner = AutoScanner::with_interval(pool, scan_state, auto_scan_interval);
        tracing::info!(
            "Auto-scan enabled with interval {} seconds",
            auto_scan_interval
        );
        Some(auto_scanner.start())
    } else {
        None
    };

    let addr = format!("0.0.0.0:{port}");
    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("Failed to bind to {}: {}", addr, e);
            tracing::error!("Is another process already using port {}?", port);
            std::process::exit(1);
        }
    };
    tracing::info!(
        "Subsonic server listening on {}",
        listener
            .local_addr()
            .expect("listener should have local addr")
    );

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("Server error: {}", e);
        std::process::exit(1);
    }
}
