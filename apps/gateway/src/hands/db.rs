//! Database queries for Hands jobs, steps, sessions, and screenshots.

use anyhow::{Context, Result};
use sqlx::{FromRow, PgPool};

// ── Row types ──────────────────────────────────────────────────────────

#[derive(Debug, FromRow)]
pub struct HandsJobRow {
    pub id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    #[sqlx(rename = "hostOS")]
    pub host_os: Option<String>,
    pub priority: i32,
    #[sqlx(rename = "maxDurationSecs")]
    pub max_duration_secs: i32,
    #[sqlx(rename = "createdAt")]
    pub created_at: chrono::NaiveDateTime,
    #[sqlx(rename = "updatedAt")]
    pub updated_at: chrono::NaiveDateTime,
    #[sqlx(rename = "completedAt")]
    pub completed_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, FromRow)]
pub struct HandsStepRow {
    pub id: String,
    #[sqlx(rename = "jobId")]
    pub job_id: String,
    #[sqlx(rename = "sequenceNumber")]
    pub sequence_number: i32,
    pub description: String,
    #[sqlx(rename = "macroInstructions")]
    pub macro_instructions: serde_json::Value,
    #[sqlx(rename = "expectedOutcome")]
    pub expected_outcome: String,
    pub status: String,
    #[sqlx(rename = "retryCount")]
    pub retry_count: i32,
    #[sqlx(rename = "maxRetries")]
    pub max_retries: i32,
    #[sqlx(rename = "timeoutMs")]
    pub timeout_ms: i32,
    #[sqlx(rename = "requireConfirm")]
    pub require_confirm: bool,
    #[sqlx(rename = "createdAt")]
    pub created_at: chrono::NaiveDateTime,
    #[sqlx(rename = "startedAt")]
    pub started_at: Option<chrono::NaiveDateTime>,
    #[sqlx(rename = "completedAt")]
    pub completed_at: Option<chrono::NaiveDateTime>,
    #[sqlx(rename = "errorMessage")]
    pub error_message: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct HandsSessionRow {
    pub id: String,
    #[sqlx(rename = "jobId")]
    pub job_id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    #[sqlx(rename = "agentToken")]
    pub agent_token: String,
    #[sqlx(rename = "browserSessionId")]
    pub browser_session_id: Option<String>,
    pub status: String,
    #[sqlx(rename = "hostOS")]
    pub host_os: Option<String>,
    #[sqlx(rename = "sessionKeyHash")]
    pub session_key_hash: Option<String>,
    #[sqlx(rename = "deviceId")]
    pub device_id: Option<String>,
    #[sqlx(rename = "createdAt")]
    pub created_at: chrono::NaiveDateTime,
    #[sqlx(rename = "lastActivityAt")]
    pub last_activity_at: chrono::NaiveDateTime,
    #[sqlx(rename = "closedAt")]
    pub closed_at: Option<chrono::NaiveDateTime>,
    #[sqlx(rename = "closeReason")]
    pub close_reason: Option<String>,
}

#[derive(Debug, FromRow)]
pub struct HandsScreenshotRow {
    pub id: String,
    #[sqlx(rename = "sessionId")]
    pub session_id: String,
    #[sqlx(rename = "stepId")]
    pub step_id: String,
    #[sqlx(rename = "capturedAt")]
    pub captured_at: chrono::NaiveDateTime,
    pub width: i32,
    pub height: i32,
    #[sqlx(rename = "sizeBytes")]
    pub size_bytes: i32,
    pub analysis: Option<serde_json::Value>,
    #[sqlx(rename = "expiresAt")]
    pub expires_at: chrono::NaiveDateTime,
}

// ── Job queries ────────────────────────────────────────────────────────

pub async fn find_job(pool: &PgPool, job_id: &str) -> Result<Option<HandsJobRow>> {
    sqlx::query_as::<_, HandsJobRow>(
        r#"SELECT * FROM "HandsJob" WHERE id = $1 LIMIT 1"#,
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await
    .context("querying HandsJob by id")
}

pub async fn find_jobs_by_user(pool: &PgPool, user_id: &str) -> Result<Vec<HandsJobRow>> {
    sqlx::query_as::<_, HandsJobRow>(
        r#"SELECT * FROM "HandsJob" WHERE "userId" = $1 ORDER BY "createdAt" DESC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("querying HandsJobs by userId")
}

pub async fn create_job(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    name: &str,
    description: &str,
    host_os: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "HandsJob" (id, "userId", name, description, status, "hostOS")
           VALUES ($1, $2, $3, $4, 'draft', $5)"#,
    )
    .bind(id)
    .bind(user_id)
    .bind(name)
    .bind(description)
    .bind(host_os)
    .execute(pool)
    .await
    .context("inserting HandsJob")?;
    Ok(())
}

pub async fn update_job_status(
    pool: &PgPool,
    job_id: &str,
    status: &str,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE "HandsJob" SET status = $2, "updatedAt" = NOW() WHERE id = $1"#,
    )
    .bind(job_id)
    .bind(status)
    .execute(pool)
    .await
    .context("updating HandsJob status")?;
    Ok(())
}

// ── Step queries ───────────────────────────────────────────────────────

pub async fn find_steps_by_job(pool: &PgPool, job_id: &str) -> Result<Vec<HandsStepRow>> {
    sqlx::query_as::<_, HandsStepRow>(
        r#"SELECT * FROM "HandsStep" WHERE "jobId" = $1 ORDER BY "sequenceNumber" ASC"#,
    )
    .bind(job_id)
    .fetch_all(pool)
    .await
    .context("querying HandsSteps by jobId")
}

pub async fn create_step(
    pool: &PgPool,
    id: &str,
    job_id: &str,
    sequence_number: i32,
    description: &str,
    macro_instructions: &serde_json::Value,
    expected_outcome: &str,
    max_retries: i32,
    timeout_ms: i32,
    require_confirm: bool,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "HandsStep"
           (id, "jobId", "sequenceNumber", description, "macroInstructions", "expectedOutcome",
            "maxRetries", "timeoutMs", "requireConfirm")
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
    )
    .bind(id)
    .bind(job_id)
    .bind(sequence_number)
    .bind(description)
    .bind(macro_instructions)
    .bind(expected_outcome)
    .bind(max_retries)
    .bind(timeout_ms)
    .bind(require_confirm)
    .execute(pool)
    .await
    .context("inserting HandsStep")?;
    Ok(())
}

pub async fn update_step_status(
    pool: &PgPool,
    step_id: &str,
    status: &str,
    error_message: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE "HandsStep"
           SET status = $2, "errorMessage" = $3, "completedAt" = CASE WHEN $2 IN ('succeeded','failed','skipped') THEN NOW() ELSE "completedAt" END
           WHERE id = $1"#,
    )
    .bind(step_id)
    .bind(status)
    .bind(error_message)
    .execute(pool)
    .await
    .context("updating HandsStep status")?;
    Ok(())
}

/// Find the next pending step for a job.
pub async fn find_next_pending_step(pool: &PgPool, job_id: &str) -> Result<Option<HandsStepRow>> {
    sqlx::query_as::<_, HandsStepRow>(
        r#"SELECT * FROM "HandsStep"
           WHERE "jobId" = $1 AND status = 'pending'
           ORDER BY "sequenceNumber" ASC
           LIMIT 1"#,
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await
    .context("querying next pending HandsStep")
}

// ── Session queries ────────────────────────────────────────────────────

pub async fn find_session(pool: &PgPool, session_id: &str) -> Result<Option<HandsSessionRow>> {
    sqlx::query_as::<_, HandsSessionRow>(
        r#"SELECT * FROM "HandsSession" WHERE id = $1 LIMIT 1"#,
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .context("querying HandsSession by id")
}

pub async fn create_session(
    pool: &PgPool,
    id: &str,
    job_id: &str,
    user_id: &str,
    agent_token: &str,
    host_os: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "HandsSession"
           (id, "jobId", "userId", "agentToken", status, "hostOS")
           VALUES ($1, $2, $3, $4, 'establishing', $5)"#,
    )
    .bind(id)
    .bind(job_id)
    .bind(user_id)
    .bind(agent_token)
    .bind(host_os)
    .execute(pool)
    .await
    .context("inserting HandsSession")?;
    Ok(())
}

pub async fn activate_session(
    pool: &PgPool,
    session_id: &str,
    browser_session_id: &str,
    device_id: Option<&str>,
    host_os: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE "HandsSession"
           SET status = 'active', "browserSessionId" = $2, "deviceId" = $3,
               "hostOS" = COALESCE($4, "hostOS"), "lastActivityAt" = NOW()
           WHERE id = $1"#,
    )
    .bind(session_id)
    .bind(browser_session_id)
    .bind(device_id)
    .bind(host_os)
    .execute(pool)
    .await
    .context("activating HandsSession")?;
    Ok(())
}

pub async fn close_session(
    pool: &PgPool,
    session_id: &str,
    reason: &str,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE "HandsSession"
           SET status = 'closed', "closedAt" = NOW(), "closeReason" = $2
           WHERE id = $1"#,
    )
    .bind(session_id)
    .bind(reason)
    .execute(pool)
    .await
    .context("closing HandsSession")?;
    Ok(())
}

pub async fn touch_session(pool: &PgPool, session_id: &str) -> Result<()> {
    sqlx::query(
        r#"UPDATE "HandsSession" SET "lastActivityAt" = NOW() WHERE id = $1"#,
    )
    .bind(session_id)
    .execute(pool)
    .await
    .context("touching HandsSession")?;
    Ok(())
}

// ── Screenshot queries ─────────────────────────────────────────────────

pub async fn find_screenshots_by_session(
    pool: &PgPool,
    session_id: &str,
) -> Result<Vec<HandsScreenshotRow>> {
    sqlx::query_as::<_, HandsScreenshotRow>(
        r#"SELECT id, "sessionId", "stepId", "capturedAt", width, height, "sizeBytes", analysis, "expiresAt"
           FROM "HandsScreenshot"
           WHERE "sessionId" = $1
           ORDER BY "capturedAt" DESC"#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await
    .context("querying HandsScreenshots by session")
}

pub async fn insert_screenshot(
    pool: &PgPool,
    id: &str,
    session_id: &str,
    step_id: &str,
    image_data: &[u8],
    width: i32,
    height: i32,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "HandsScreenshot"
           (id, "sessionId", "stepId", "imageData", width, height, "sizeBytes", "expiresAt")
           VALUES ($1, $2, $3, $4, $5, $6, $7, NOW() + INTERVAL '24 hours')"#,
    )
    .bind(id)
    .bind(session_id)
    .bind(step_id)
    .bind(image_data)
    .bind(width)
    .bind(height)
    .bind(image_data.len() as i32)
    .execute(pool)
    .await
    .context("inserting HandsScreenshot")?;
    Ok(())
}

// ── Audit queries ──────────────────────────────────────────────────────

pub async fn insert_audit_event(
    pool: &PgPool,
    id: &str,
    job_id: &str,
    event: &str,
    step_id: Option<&str>,
    session_id: Option<&str>,
    metadata: Option<&serde_json::Value>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "HandsAuditEvent"
           (id, "jobId", event, "stepId", "sessionId", metadata)
           VALUES ($1, $2, $3, $4, $5, $6)"#,
    )
    .bind(id)
    .bind(job_id)
    .bind(event)
    .bind(step_id)
    .bind(session_id)
    .bind(metadata)
    .execute(pool)
    .await
    .context("inserting HandsAuditEvent")?;
    Ok(())
}

/// Count steps for a job (used in list view).
pub async fn count_steps(pool: &PgPool, job_id: &str) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        r#"SELECT COUNT(*) FROM "HandsStep" WHERE "jobId" = $1"#,
    )
    .bind(job_id)
    .fetch_one(pool)
    .await
    .context("counting steps")?;
    Ok(row.0)
}
