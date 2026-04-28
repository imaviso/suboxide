//! Remote control session and command persistence.

use chrono::{Duration, NaiveDateTime, Utc};
use diesel::prelude::*;
use diesel::sql_types::{BigInt, Integer, Nullable, Text, Timestamp};
use rand_core::{OsRng, RngCore};

use crate::db::DbPool;

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
            diesel::sql_query(
                "UPDATE remote_sessions
                 SET closed_at = ?, updated_at = ?
                 WHERE owner_user_id = ? AND host_device_id = ? AND closed_at IS NULL",
            )
            .bind::<Timestamp, _>(now)
            .bind::<Timestamp, _>(now)
            .bind::<Integer, _>(owner_user_id)
            .bind::<Text, _>(host_device_id)
            .execute(conn)?;

            for _ in 0..5 {
                let session_id = generate_session_id();
                let pairing_code = generate_pairing_code();

                let insert_result = diesel::sql_query(
                    "INSERT INTO remote_sessions (
                        session_id,
                        pairing_code,
                        owner_user_id,
                        host_device_id,
                        host_device_name,
                        expires_at,
                        created_at,
                        updated_at
                     )
                     VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                )
                .bind::<Text, _>(&session_id)
                .bind::<Text, _>(&pairing_code)
                .bind::<Integer, _>(owner_user_id)
                .bind::<Text, _>(host_device_id)
                .bind::<Nullable<Text>, _>(host_device_name)
                .bind::<Timestamp, _>(expires_at)
                .bind::<Timestamp, _>(now)
                .bind::<Timestamp, _>(now)
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

        let session_row = diesel::sql_query(
            "SELECT
                session_id,
                pairing_code,
                owner_user_id,
                host_device_id,
                host_device_name,
                controller_user_id,
                controller_device_id,
                controller_device_name,
                expires_at,
                created_at,
                updated_at,
                closed_at
             FROM remote_sessions
             WHERE pairing_code = ?
               AND closed_at IS NULL
               AND expires_at > CURRENT_TIMESTAMP
             LIMIT 1",
        )
        .bind::<Text, _>(pairing_code)
        .get_result::<RemoteSessionRow>(&mut conn)
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

        let changed = diesel::sql_query(
            "UPDATE remote_sessions
             SET
               controller_user_id = ?,
               controller_device_id = ?,
               controller_device_name = ?,
               pairing_code = ?,
               expires_at = ?,
               updated_at = ?
              WHERE session_id = ?
                AND pairing_code = ?
                AND owner_user_id = ?
                AND expires_at > ?
                AND closed_at IS NULL
                AND controller_user_id IS NULL",
        )
        .bind::<Integer, _>(controller_user_id)
        .bind::<Text, _>(controller_device_id)
        .bind::<Nullable<Text>, _>(controller_device_name)
        .bind::<Text, _>(&consumed_pairing_code)
        .bind::<Timestamp, _>(new_expiry)
        .bind::<Timestamp, _>(now)
        .bind::<Text, _>(&session_row.session_id)
        .bind::<Text, _>(pairing_code)
        .bind::<Integer, _>(controller_user_id)
        .bind::<Timestamp, _>(now)
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
        let row = diesel::sql_query(
            "SELECT
                session_id,
                pairing_code,
                owner_user_id,
                host_device_id,
                host_device_name,
                controller_user_id,
                controller_device_id,
                controller_device_name,
                expires_at,
                created_at,
                updated_at,
                closed_at
             FROM remote_sessions
             WHERE session_id = ?
               AND closed_at IS NULL
               AND expires_at > CURRENT_TIMESTAMP
               AND (owner_user_id = ? OR controller_user_id = ?)
             LIMIT 1",
        )
        .bind::<Text, _>(session_id)
        .bind::<Integer, _>(user_id)
        .bind::<Integer, _>(user_id)
        .get_result::<RemoteSessionRow>(&mut conn)
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

        let changed = diesel::sql_query(
            "UPDATE remote_sessions
             SET closed_at = ?, updated_at = ?
             WHERE session_id = ?
               AND closed_at IS NULL
               AND (owner_user_id = ? OR controller_user_id = ?)",
        )
        .bind::<Timestamp, _>(now)
        .bind::<Timestamp, _>(now)
        .bind::<Text, _>(session_id)
        .bind::<Integer, _>(user_id)
        .bind::<Integer, _>(user_id)
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

        let changed = diesel::sql_query(
            "INSERT INTO remote_commands (session_id, source_device_id, command, payload, created_at)
             SELECT ?, ?, ?, ?, ?
             WHERE EXISTS (
                 SELECT 1 FROM remote_sessions
                 WHERE session_id = ?
                   AND closed_at IS NULL
                   AND expires_at > CURRENT_TIMESTAMP
             )",
        )
        .bind::<Text, _>(session_id)
        .bind::<Text, _>(source_device_id)
        .bind::<Text, _>(command)
        .bind::<Nullable<Text>, _>(payload)
        .bind::<Timestamp, _>(now)
        .bind::<Text, _>(session_id)
        .execute(&mut conn)?;

        if changed == 0 {
            return Err(MusicRepoError::new(
                MusicRepoErrorKind::NotFound,
                "remote session not found or inactive",
            ));
        }

        let row = diesel::sql_query("SELECT last_insert_rowid() AS id")
            .get_result::<LastInsertRow>(&mut conn)?;

        Ok(row.id)
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

        let rows = diesel::sql_query(
            "SELECT id, command, payload, source_device_id, created_at
             FROM remote_commands
             WHERE session_id = ?
               AND id > ?
               AND source_device_id != ?
             ORDER BY id ASC
             LIMIT ?",
        )
        .bind::<Text, _>(session_id)
        .bind::<BigInt, _>(since_id)
        .bind::<Text, _>(exclude_device_id)
        .bind::<BigInt, _>(limit)
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

        let changed = diesel::sql_query(
            "INSERT INTO remote_state (session_id, state_json, updated_by_device_id, updated_at)
             SELECT ?, ?, ?, ?
             WHERE EXISTS (
                 SELECT 1 FROM remote_sessions
                 WHERE session_id = ?
                   AND closed_at IS NULL
                   AND expires_at > CURRENT_TIMESTAMP
             )
             ON CONFLICT(session_id) DO UPDATE
             SET
                 state_json = excluded.state_json,
                 updated_by_device_id = excluded.updated_by_device_id,
                 updated_at = excluded.updated_at",
        )
        .bind::<Text, _>(session_id)
        .bind::<Text, _>(state_json)
        .bind::<Text, _>(updated_by_device_id)
        .bind::<Timestamp, _>(now)
        .bind::<Text, _>(session_id)
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
        let row = diesel::sql_query(
            "SELECT state_json, updated_by_device_id, updated_at
             FROM remote_state
             WHERE session_id = ?
             LIMIT 1",
        )
        .bind::<Text, _>(session_id)
        .get_result::<RemoteStateRow>(&mut conn)
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
        diesel::sql_query(
            "SELECT
                session_id,
                pairing_code,
                owner_user_id,
                host_device_id,
                host_device_name,
                controller_user_id,
                controller_device_id,
                controller_device_name,
                expires_at,
                created_at,
                updated_at,
                closed_at
             FROM remote_sessions
             WHERE session_id = ?
             LIMIT 1",
        )
        .bind::<Text, _>(session_id)
        .get_result::<RemoteSessionRow>(conn)
        .map(RemoteSession::from)
        .map_err(MusicRepoError::from)
    }
}

#[derive(QueryableByName)]
struct LastInsertRow {
    #[diesel(sql_type = BigInt)]
    id: i64,
}

#[derive(QueryableByName)]
struct RemoteSessionRow {
    #[diesel(sql_type = Text)]
    session_id: String,
    #[diesel(sql_type = Text)]
    pairing_code: String,
    #[diesel(sql_type = Integer)]
    owner_user_id: i32,
    #[diesel(sql_type = Text)]
    host_device_id: String,
    #[diesel(sql_type = Nullable<Text>)]
    host_device_name: Option<String>,
    #[diesel(sql_type = Nullable<Integer>)]
    controller_user_id: Option<i32>,
    #[diesel(sql_type = Nullable<Text>)]
    controller_device_id: Option<String>,
    #[diesel(sql_type = Nullable<Text>)]
    controller_device_name: Option<String>,
    #[diesel(sql_type = Timestamp)]
    expires_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    created_at: NaiveDateTime,
    #[diesel(sql_type = Timestamp)]
    updated_at: NaiveDateTime,
    #[diesel(sql_type = Nullable<Timestamp>)]
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

#[derive(QueryableByName)]
struct RemoteCommandRow {
    #[diesel(sql_type = BigInt)]
    id: i64,
    #[diesel(sql_type = Text)]
    command: String,
    #[diesel(sql_type = Nullable<Text>)]
    payload: Option<String>,
    #[diesel(sql_type = Text)]
    source_device_id: String,
    #[diesel(sql_type = Timestamp)]
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

#[derive(QueryableByName)]
struct RemoteStateRow {
    #[diesel(sql_type = Text)]
    state_json: String,
    #[diesel(sql_type = Text)]
    updated_by_device_id: String,
    #[diesel(sql_type = Timestamp)]
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
