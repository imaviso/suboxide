# AGENTS.md

Project status: pre-release alpha. Do not preserve backward compatibility unless
explicitly requested. We will prefer clean, breaking internal refactors over
compatibility layers, graceful degradation, and framework bridges that keep old
patterns alive. Keep it simple, no bloat, no extra abstractions, no future
proofing, no defensive code. Take hard-path if it results better, correct,
idiomatic, and elegant code.

suboxide: Rust 2024 Subsonic/OpenSubsonic music server.
## Stack
- Axum 0.8, Tokio; SQLite via Diesel+r2d2; Serde JSON, quick-xml XML.
- Argon2, MD5 token auth, API keys; reqwest+scraper Last.fm client.
- lofty+walkdir+rayon scanner; clap CLI; mimalloc global alloc.
## Commands
```bash
cargo build
cargo test
cargo test test_name
cargo fmt
cargo clippy
cargo check
cargo run -- serve
RUST_LOG=suboxide=debug cargo run -- serve
```
- Before finish: relevant tests, `cargo fmt`, `cargo clippy -- -D warnings`.
## Skills
- Always load `rust-coding-skill` and `axum-web-framework` when writing or refactoring Rust code.
## Layout
- `src/main.rs`: CLI, database setup, server config, command runners.
- `src/lib.rs`: crate root; re-exports api, app, crypto, db, lastfm, models, paths, scanner.
- `src/app.rs`: `AppState`, `create_router` — wires all handlers and shared state.
- `src/api/`: auth, error, response, router, services (MusicLibrary, Users, RemoteSessions).
- `src/api/handlers/`: endpoint groups — annotation, media, playlists, playqueue, remote, scanning, system, users, browsing/.
- `src/api/handlers/browsing/`: directory, indexes, info, lists, retrieval, search.
- `src/crypto/`: Argon2 password hashing and verification.
- `src/db/`: connection (pool, migrations), schema, repo/ (user, music, interaction, playlist, remote, artist_cache, error).
- `src/lastfm/`: reqwest-based Last.fm API client (scrobble, now-playing, artist info).
- `src/models/`: user, music (domain types + Subsonic XML/JSON response structs).
- `src/paths.rs`: cover art directory resolution.
- `src/scanner/`: engine (discovery, ingest, auto-scan loop), state (progress, RAII guard), lyrics (LRC parser), types.
