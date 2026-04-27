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
- Argon2, MD5 token auth, API keys; lofty+walkdir scanner; clap CLI.
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
- Before finish: relevant tests, `cargo fmt`, `cargo clippy` no warnings.
## Layout
- `src/main.rs`: CLI, app state, router setup.
- `src/api/{auth,error,response,router}.rs`: auth, `ApiError`, XML/JSON, routes.
- `src/api/handlers/`: endpoints; `src/db/`: Diesel repos/schema; `src/models/`: domain/responses; `src/scanner/`: scanner.

## Rust Rules
- Imports: std, external crates, `crate::`; explicit over glob.
- Names: precise nouns; avoid vague `Manager`/`Factory`/`Service` suffixes.
- Public items need `Debug` and `///`; first doc sentence under 15 words; examples when practical.
- User-facing types implement `Display`; responses end `Response`; inserts start `New`.
- Prefer `PathBuf`, ID newtypes, `impl AsRef<str/path>` inputs, builders for 4+ params.
- No runtime `unwrap()`/`expect()`; panic only for impossible contract violations.
- No `unsafe` unless FFI/novel abstraction; document invariants fully.
- Use typed `std::error::Error` structs for library-style errors.
- Use `#[expect(...)]`, not `#[allow(...)]`, for justified lint overrides.
- Do not expose `Arc`/`Rc`/`RefCell` publicly; keep shared components cheap to clone.
- Keep I/O mockable; decouple parsing/core logic from files/sockets.
- Use structured `tracing` fields, not formatted log strings.

## API Rules
- Handler args: `Query<T>` first, then `SubsonicAuth`; return `impl IntoResponse`.
- Use `ok_*` helpers or `error_response(auth.format, &ApiError::...)`.
- Every endpoint supports XML and JSON; update both response modules.
- XML attrs use `#[serde(rename = "@name")]`; text uses `$text`; JSON camelCase.
- POST currently reads query strings, not form bodies. Do not advertise `formPost` until body parsing exists.
- Register only truly supported OpenSubsonic extensions in `supported_extensions()`.

## DB Changes
- Use repos in `src/db/`; migrations in `migrations/`; never edit generated `src/db/schema.rs`.
- New endpoint: handler, route in `create_router()`, response/AuthState/repo methods as needed.
- New table: `diesel migration generate`, write `up.sql`/`down.sql`, run migration, add models/repos.
