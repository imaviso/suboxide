//! Remote control session and command persistence.

use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use rand_core::{OsRng, RngCore};

use crate::db::DbPool;
use crate::db::schema::{remote_commands, remote_sessions, remote_state};

use super::error::{MusicRepoError, MusicRepoErrorKind};

const DEFAULT_JOINED_SESSION_TTL_SECONDS: i64 = 60 * 60 * 12;

/// A remote-control session connecting a host player and a controller device.
#[derive(Debug, Clone)]
pub struct RemoteSession {
    pub session_id: String,
    pub pairing_code: String,
    pub owner_user_id: i32,
    pub host_device_id: String,
    pub host_device_name: Option<String>,
    pub controller_user_id: Option<i32>,
    pub controller_device_id: Option<String>,
    pub controller_device_name: Option<String>,
    pub expires_at: NaiveDateTime,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
    pub closed_at: Option<NaiveDateTime>,
}

/// A command queued for remote playback control.
#[derive(Debug, Clone)]
pub struct RemoteCommand {
    pub id: i64,
    pub command: String,
    pub payload: Option<String>,
    pub source_device_id: String,
    pub created_at: NaiveDateTime,
}

/// The latest remote playback state reported by a host device.
#[derive(Debug, Clone)]
pub struct RemoteState {
    pub state_json: String,
    pub updated_by_device_id: String,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = remote_sessions)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[expect(
    dead_code,
    reason = "full-table projection; id is not part of the domain type"
)]
struct RemoteSessionRow {
    id: i32,
    session_id: String,
    pairing_code: String,
    owner_user_id: i32,
    host_device_id: String,
    host_device_name: Option<String>,
    controller_user_id: Option<i32>,
    controller_device_id: Option<String>,
    controller_device_name: Option<String>,
    expires_at: NaiveDateTime,
    created_at: NaiveDateTime,
    updated_at: NaiveDateTime,
    closed_at: Option<NaiveDateTime>,
}

impl From<RemoteSessionRow> for RemoteSession {
    fn from(row: RemoteSessionRow) -> Self {
        Self {
            session_id: row.session_id,
            pairing_code: row.pairing_code,
            owner_user_id: row.owner_user_id,
            host_device_id: row.host_device_id,
            host_device_name: row.host_device_name,
            controller_user_id: row.controller_user_id,
            controller_device_id: row.controller_device_id,
            controller_device_name: row.controller_device_name,
            expires_at: row.expires_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
            closed_at: row.closed_at,
        }
    }
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = remote_commands)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[expect(
    dead_code,
    reason = "full-table projection; session_id is not part of the domain type"
)]
struct RemoteCommandRow {
    id: i64,
    session_id: String,
    source_device_id: String,
    command: String,
    payload: Option<String>,
    created_at: NaiveDateTime,
}

impl From<RemoteCommandRow> for RemoteCommand {
    fn from(row: RemoteCommandRow) -> Self {
        Self {
            id: row.id,
            command: row.command,
            payload: row.payload,
            source_device_id: row.source_device_id,
            created_at: row.created_at,
        }
    }
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[diesel(table_name = remote_state)]
#[diesel(check_for_backend(diesel::sqlite::Sqlite))]
#[expect(
    dead_code,
    reason = "full-table projection; session_id is not part of the domain type"
)]
struct RemoteStateRow {
    session_id: String,
    state_json: String,
    updated_by_device_id: String,
    updated_at: NaiveDateTime,
}

impl From<RemoteStateRow> for RemoteState {
    fn from(row: RemoteStateRow) -> Self {
        Self {
            state_json: row.state_json,
            updated_by_device_id: row.updated_by_device_id,
            updated_at: row.updated_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RemoteControlRepository {
    pool: DbPool,
}

impl RemoteControlRepository {
    /// Create a new remote control repository.
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    /// Create a remote session for a host device.
    ///
    /// # Errors
    /// Returns an error if database access fails.
    pub fn create_session(
        &self,
        owner_user_id: i32,
        host_device_id: &str,
        host_device_name: Option<&str>,
        ttl_seconds: i64,
    ) -> Result<RemoteSession, MusicRepoError> {
        let mut conn = self.pool.get()?;
        let now = Utc::now().naive_utc();
        let expires_at = now + Duration::seconds(ttl_seconds);

        conn.transaction(|conn| {
            diesel::update(remote_sessions::table)
                .filter(remote_sessions::owner_user_id.eq(owner_user_id))
                .filter(remote_sessions::host_device_id.eq(host_device_id))
                .filter(remote_sessions::closed_at.is_null())
                .set((
                    remote_sessions::closed_at.eq(Some(now)),
                    remote_sessions::updated_at.eq(now),
                ))
                .execute(conn)?;

            for _ in 0..5 {
                let session_id = generate_session_id();
                let pairing_code = generate_pairing_code();

                let insert_result = diesel::insert_into(remote_sessions::table)
                    .values((
                        remote_sessions::session_id.eq(&session_id),
                        remote_sessions::pairing_code.eq(&pairing_code),
                        remote_sessions::owner_user_id.eq(owner_user_id),
                        remote_sessions::host_device_id.eq(host_device_id),
                        remote_sessions::host_device_name.eq(host_device_name),
                        remote_sessions::expires_at.eq(expires_at),
                        remote_sessions::created_at.eq(now),
                        remote_sessions::updated_at.eq(now),
                    ))
                    .execute(conn);

                match insert_result {
                    Ok(_) => return Self::get_session_by_id_with_conn(conn, &session_id),
                    Err(diesel::result::Error::DatabaseError(
                        diesel::result::DatabaseErrorKind::UniqueViolation,
                        _,
                    )) => {}
                    Err(error) => return Err(error.into()),
                }
            }

            Err(MusicRepoError::new(
                MusicRepoErrorKind::Database,
                "failed to create unique remote session",
            ))
        })
    }

    /// Join a remote session using a pairing code.
    ///
    /// Returns `Ok(None)` when the code is invalid, expired, or not authorized.
    ///
    /// # Errors
    /// Returns an error if database access fails.
    pub fn join_session(
        &self,
        pairing_code: &str,
        controller_user_id: i32,
        controller_device_id: &str,
        controller_device_name: Option<&str>,
    ) -> Result<Option<RemoteSession>, MusicRepoError> {
        let mut conn = self.pool.get()?;
        let now = Utc::now().naive_utc();

        let session_row = remote_sessions::table
            .filter(remote_sessions::pairing_code.eq(pairing_code))
            .filter(remote_sessions::closed_at.is_null())
            .filter(remote_sessions::expires_at.gt(now))
            .select(RemoteSessionRow::as_select())
            .first::<RemoteSessionRow>(&mut conn)
            .optional()?;

        let Some(session_row) = session_row else {
            return Ok(None);
        };

        // Same-user only pairing for now.
        if session_row.owner_user_id != controller_user_id {
            return Ok(None);
        }

        let new_expiry = now + Duration::seconds(DEFAULT_JOINED_SESSION_TTL_SECONDS);
        let consumed_pairing_code = format!("joined-{}", session_row.session_id);

        let changed = diesel::update(remote_sessions::table)
            .filter(remote_sessions::session_id.eq(&session_row.session_id))
            .filter(remote_sessions::pairing_code.eq(pairing_code))
            .filter(remote_sessions::owner_user_id.eq(controller_user_id))
            .filter(remote_sessions::expires_at.gt(now))
            .filter(remote_sessions::closed_at.is_null())
            .filter(remote_sessions::controller_user_id.is_null())
            .set((
                remote_sessions::controller_user_id.eq(Some(controller_user_id)),
                remote_sessions::controller_device_id.eq(Some(controller_device_id)),
                remote_sessions::controller_device_name.eq(controller_device_name),
                remote_sessions::pairing_code.eq(&consumed_pairing_code),
                remote_sessions::expires_at.eq(new_expiry),
                remote_sessions::updated_at.eq(now),
            ))
            .execute(&mut conn)?;

        if changed == 0 {
            return Ok(None);
        }

        self.get_session_by_id(&session_row.session_id).map(Some)
    }

    /// Get an active session visible to a specific user.
    ///
    /// # Errors
    /// Returns an error if database access fails.
    pub fn get_session_for_user(
        &self,
        session_id: &str,
        user_id: i32,
    ) -> Result<Option<RemoteSession>, MusicRepoError> {
        let mut conn = self.pool.get()?;
        let now = Utc::now().naive_utc();

        let row = remote_sessions::table
            .filter(remote_sessions::session_id.eq(session_id))
            .filter(remote_sessions::closed_at.is_null())
            .filter(remote_sessions::expires_at.gt(now))
            .filter(
                remote_sessions::owner_user_id
                    .eq(user_id)
                    .or(remote_sessions::controller_user_id.eq(user_id)),
            )
            .select(RemoteSessionRow::as_select())
            .first::<RemoteSessionRow>(&mut conn)
            .optional()?;

        Ok(row.map(RemoteSession::from))
    }

    /// Close an active remote session.
    ///
    /// # Errors
    /// Returns an error if database access fails.
    pub fn close_session(&self, session_id: &str, user_id: i32) -> Result<bool, MusicRepoError> {
        let mut conn = self.pool.get()?;
        let now = Utc::now().naive_utc();

        let changed = diesel::update(remote_sessions::table)
            .filter(remote_sessions::session_id.eq(session_id))
            .filter(remote_sessions::closed_at.is_null())
            .filter(
                remote_sessions::owner_user_id
                    .eq(user_id)
                    .or(remote_sessions::controller_user_id.eq(user_id)),
            )
            .set((
                remote_sessions::closed_at.eq(Some(now)),
                remote_sessions::updated_at.eq(now),
            ))
            .execute(&mut conn)?;

        Ok(changed > 0)
    }

    /// Queue a command for a remote session.
    ///
    /// # Errors
    /// Returns an error if the session is not active or database access fails.
    pub fn enqueue_command(
        &self,
        session_id: &str,
        source_device_id: &str,
        command: &str,
        payload: Option<&str>,
    ) -> Result<i64, MusicRepoError> {
        let mut conn = self.pool.get()?;
        let now = Utc::now().naive_utc();

        let session_active = diesel::select(diesel::dsl::exists(
            remote_sessions::table
                .filter(remote_sessions::session_id.eq(session_id))
                .filter(remote_sessions::closed_at.is_null())
                .filter(remote_sessions::expires_at.gt(now)),
        ))
        .get_result::<bool>(&mut conn)?;

        if !session_active {
            return Err(MusicRepoError::new(
                MusicRepoErrorKind::NotFound,
                "remote session not found or inactive",
            ));
        }

        diesel::insert_into(remote_commands::table)
            .values((
                remote_commands::session_id.eq(session_id),
                remote_commands::source_device_id.eq(source_device_id),
                remote_commands::command.eq(command),
                remote_commands::payload.eq(payload),
                remote_commands::created_at.eq(now),
            ))
            .execute(&mut conn)?;

        let row = remote_commands::table
            .select(remote_commands::id)
            .order(remote_commands::id.desc())
            .first::<i64>(&mut conn)?;

        Ok(row)
    }

    /// Get queued commands after a command id.
    ///
    /// # Errors
    /// Returns an error if database access fails.
    pub fn get_commands(
        &self,
        session_id: &str,
        since_id: i64,
        limit: i64,
        exclude_device_id: &str,
    ) -> Result<Vec<RemoteCommand>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let rows = remote_commands::table
            .filter(remote_commands::session_id.eq(session_id))
            .filter(remote_commands::id.gt(since_id))
            .filter(remote_commands::source_device_id.ne(exclude_device_id))
            .order(remote_commands::id.asc())
            .limit(limit)
            .select(RemoteCommandRow::as_select())
            .load::<RemoteCommandRow>(&mut conn)?;

        Ok(rows.into_iter().map(RemoteCommand::from).collect())
    }

    /// Upsert the latest remote state payload for a session.
    ///
    /// # Errors
    /// Returns an error if database access fails.
    pub fn update_state(
        &self,
        session_id: &str,
        updated_by_device_id: &str,
        state_json: &str,
    ) -> Result<(), MusicRepoError> {
        let mut conn = self.pool.get()?;
        let now = Utc::now().naive_utc();

        let changed = diesel::insert_into(remote_state::table)
            .values((
                remote_state::session_id.eq(session_id),
                remote_state::state_json.eq(state_json),
                remote_state::updated_by_device_id.eq(updated_by_device_id),
                remote_state::updated_at.eq(now),
            ))
            .on_conflict(remote_state::session_id)
            .do_update()
            .set((
                remote_state::state_json.eq(state_json),
                remote_state::updated_by_device_id.eq(updated_by_device_id),
                remote_state::updated_at.eq(now),
            ))
            .execute(&mut conn)?;

        if changed == 0 {
            return Err(MusicRepoError::new(
                MusicRepoErrorKind::NotFound,
                "remote session not found or inactive",
            ));
        }

        Ok(())
    }

    /// Get the latest remote state for a session.
    ///
    /// # Errors
    /// Returns an error if database access fails.
    pub fn get_state(&self, session_id: &str) -> Result<Option<RemoteState>, MusicRepoError> {
        let mut conn = self.pool.get()?;

        let row = remote_state::table
            .filter(remote_state::session_id.eq(session_id))
            .select(RemoteStateRow::as_select())
            .first::<RemoteStateRow>(&mut conn)
            .optional()?;

        Ok(row.map(RemoteState::from))
    }

    fn get_session_by_id(&self, session_id: &str) -> Result<RemoteSession, MusicRepoError> {
        let mut conn = self.pool.get()?;
        Self::get_session_by_id_with_conn(&mut conn, session_id)
    }

    fn get_session_by_id_with_conn(
        conn: &mut diesel::SqliteConnection,
        session_id: &str,
    ) -> Result<RemoteSession, MusicRepoError> {
        remote_sessions::table
            .filter(remote_sessions::session_id.eq(session_id))
            .select(RemoteSessionRow::as_select())
            .first::<RemoteSessionRow>(conn)
            .map(RemoteSession::from)
            .map_err(MusicRepoError::from)
    }
}

fn generate_session_id() -> String {
    let mut bytes = [0_u8; 16];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn generate_pairing_code() -> String {
    let mut bytes = [0_u8; 4];
    OsRng.fill_bytes(&mut bytes);
    let value = u32::from_le_bytes(bytes) % 1_000_000;
    format!("{value:06}")
}
