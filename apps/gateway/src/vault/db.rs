//! Database queries for vault records and approval requests.
//!
//! Read-only queries run by the gateway. Writes happen via the Next.js app
//! (Prisma) or via specific vault API endpoints.

use anyhow::{Context, Result};
use sqlx::{FromRow, PgPool};

// ── Row types ───────────────────────────────────────────────────────────

/// A vault record row from the `VaultRecord` table.
#[derive(Debug, FromRow)]
pub struct VaultRecordRow {
    pub id: String,
    #[sqlx(rename = "userId")]
    pub user_id: String,
    pub name: String,
    #[sqlx(rename = "recordType")]
    pub record_type: String,

    // Encrypted payload
    #[sqlx(rename = "ciphertextB64")]
    pub ciphertext_b64: String,
    #[sqlx(rename = "nonceB64")]
    pub nonce_b64: String,
    #[sqlx(rename = "aadJson")]
    pub aad_json: String,

    // Wrapped record key
    #[sqlx(rename = "wrappedKeyB64")]
    pub wrapped_key_b64: String,
    #[sqlx(rename = "wrappedKeyNonceB64")]
    pub wrapped_key_nonce_b64: String,
    #[sqlx(rename = "wrapAlg")]
    pub wrap_alg: String,

    // OneCLI public key and derivation context
    #[sqlx(rename = "onecliPubkeyJwk")]
    pub onecli_pubkey_jwk: String,
    #[sqlx(rename = "derivationContext")]
    pub derivation_context: String,

    // Policy fields
    #[sqlx(rename = "requireOnlykey")]
    pub require_onlykey: bool,
    #[sqlx(rename = "unlockTtlSeconds")]
    pub unlock_ttl_seconds: i32,
    #[sqlx(rename = "idleTimeoutSeconds")]
    pub idle_timeout_seconds: i32,
    #[sqlx(rename = "cacheScope")]
    pub cache_scope: String,
    #[sqlx(rename = "allowManualRevoke")]
    pub allow_manual_revoke: bool,
    #[sqlx(rename = "relockOnBrowserDisconnect")]
    pub relock_on_browser_disconnect: bool,
    #[sqlx(rename = "relockOnPolicyChange")]
    pub relock_on_policy_change: bool,
    #[sqlx(rename = "requireFreshUnlockForHighRisk")]
    pub require_fresh_unlock_for_high_risk: bool,
    #[sqlx(rename = "allowPlaintextReturn")]
    pub allow_plaintext_return: bool,
    #[sqlx(rename = "allowedAgents")]
    pub allowed_agents: Option<String>,

    // Versioning
    #[sqlx(rename = "recordVersion")]
    pub record_version: i32,
    #[sqlx(rename = "policyVersion")]
    pub policy_version: i32,
    #[sqlx(rename = "keyVersion")]
    pub key_version: i32,
    #[sqlx(rename = "unlockGeneration")]
    pub unlock_generation: i32,

    // Injection config
    #[sqlx(rename = "hostPattern")]
    pub host_pattern: String,
    #[sqlx(rename = "pathPattern")]
    pub path_pattern: Option<String>,
    #[sqlx(rename = "injectionConfig")]
    pub injection_config: Option<serde_json::Value>,
}

/// A pending approval request row.
#[derive(Debug, FromRow)]
pub struct VaultApprovalRow {
    pub id: String,
    #[sqlx(rename = "recordId")]
    pub record_id: String,
    #[sqlx(rename = "agentId")]
    pub agent_id: String,
    #[sqlx(rename = "sessionId")]
    pub session_id: Option<String>,
    pub operation: String,
    pub status: String,
    pub origin: String,
    #[sqlx(rename = "nonceB64")]
    pub nonce_b64: String,
    #[sqlx(rename = "browserSessionId")]
    pub browser_session_id: Option<String>,
    #[sqlx(rename = "createdAt")]
    pub created_at: chrono::NaiveDateTime,
    #[sqlx(rename = "expiresAt")]
    pub expires_at: chrono::NaiveDateTime,
}

// ── Queries ─────────────────────────────────────────────────────────────

/// Find a vault record by ID.
pub async fn find_vault_record(pool: &PgPool, record_id: &str) -> Result<Option<VaultRecordRow>> {
    sqlx::query_as::<_, VaultRecordRow>(
        r#"SELECT * FROM "VaultRecord" WHERE id = $1 LIMIT 1"#,
    )
    .bind(record_id)
    .fetch_optional(pool)
    .await
    .context("querying VaultRecord by id")
}

/// Find all vault records for a user.
pub async fn find_vault_records_by_user(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<VaultRecordRow>> {
    sqlx::query_as::<_, VaultRecordRow>(
        r#"SELECT * FROM "VaultRecord" WHERE "userId" = $1 ORDER BY "createdAt" DESC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("querying VaultRecords by userId")
}

/// Find vault records matching a hostname for a user.
pub async fn find_vault_records_by_host(
    pool: &PgPool,
    user_id: &str,
    hostname: &str,
) -> Result<Vec<VaultRecordRow>> {
    // We load all records for the user and filter in Rust (same pattern as connect.rs)
    // to support wildcard host patterns like "*.example.com"
    let all = find_vault_records_by_user(pool, user_id).await?;
    Ok(all
        .into_iter()
        .filter(|r| host_matches(hostname, &r.host_pattern))
        .collect())
}

/// Find pending approval requests (status = "pending", not expired).
pub async fn find_pending_approvals(
    pool: &PgPool,
    user_id: &str,
) -> Result<Vec<(VaultApprovalRow, VaultRecordRow)>> {
    // Join approval requests with their vault records, filtered by user
    let approvals = sqlx::query_as::<_, VaultApprovalRow>(
        r#"SELECT a.* FROM "VaultApprovalRequest" a
           INNER JOIN "VaultRecord" r ON a."recordId" = r.id
           WHERE r."userId" = $1 AND a.status = 'pending' AND a."expiresAt" > NOW()
           ORDER BY a."createdAt" DESC"#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("querying pending vault approvals")?;

    let mut results = Vec::new();
    for approval in approvals {
        if let Some(record) = find_vault_record(pool, &approval.record_id).await? {
            results.push((approval, record));
        }
    }

    Ok(results)
}

/// Find a specific pending approval by request ID.
pub async fn find_approval_by_id(
    pool: &PgPool,
    request_id: &str,
) -> Result<Option<VaultApprovalRow>> {
    sqlx::query_as::<_, VaultApprovalRow>(
        r#"SELECT * FROM "VaultApprovalRequest" WHERE id = $1 LIMIT 1"#,
    )
    .bind(request_id)
    .fetch_optional(pool)
    .await
    .context("querying VaultApprovalRequest by id")
}

/// Create a pending approval request.
pub async fn create_approval_request(
    pool: &PgPool,
    id: &str,
    record_id: &str,
    agent_id: &str,
    session_id: Option<&str>,
    operation: &str,
    origin: &str,
    nonce_b64: &str,
    expires_at: chrono::NaiveDateTime,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "VaultApprovalRequest"
           (id, "recordId", "agentId", "sessionId", operation, status, origin, "nonceB64", "expiresAt")
           VALUES ($1, $2, $3, $4, $5, 'pending', $6, $7, $8)"#,
    )
    .bind(id)
    .bind(record_id)
    .bind(agent_id)
    .bind(session_id)
    .bind(operation)
    .bind(origin)
    .bind(nonce_b64)
    .bind(expires_at)
    .execute(pool)
    .await
    .context("inserting VaultApprovalRequest")?;

    Ok(())
}

/// Update approval request status.
pub async fn update_approval_status(
    pool: &PgPool,
    request_id: &str,
    status: &str,
    browser_session_id: Option<&str>,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE "VaultApprovalRequest"
           SET status = $2, "browserSessionId" = $3
           WHERE id = $1"#,
    )
    .bind(request_id)
    .bind(status)
    .bind(browser_session_id)
    .execute(pool)
    .await
    .context("updating VaultApprovalRequest status")?;

    Ok(())
}

/// Insert an audit event.
pub async fn insert_audit_event(
    pool: &PgPool,
    id: &str,
    record_id: &str,
    event: &str,
    agent_id: Option<&str>,
    session_id: Option<&str>,
    scope_type: Option<&str>,
    scope_id: Option<&str>,
    reason: Option<&str>,
    metadata: Option<&serde_json::Value>,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO "VaultAuditEvent"
           (id, "recordId", event, "agentId", "sessionId", "scopeType", "scopeId", reason, metadata)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
    )
    .bind(id)
    .bind(record_id)
    .bind(event)
    .bind(agent_id)
    .bind(session_id)
    .bind(scope_type)
    .bind(scope_id)
    .bind(reason)
    .bind(metadata)
    .execute(pool)
    .await
    .context("inserting VaultAuditEvent")?;

    Ok(())
}

/// Expire stale pending approval requests.
pub async fn expire_stale_approvals(pool: &PgPool) -> Result<u64> {
    let result = sqlx::query(
        r#"UPDATE "VaultApprovalRequest"
           SET status = 'expired'
           WHERE status = 'pending' AND "expiresAt" <= NOW()"#,
    )
    .execute(pool)
    .await
    .context("expiring stale approvals")?;

    Ok(result.rows_affected())
}

// ── Host matching (shared with connect.rs) ──────────────────────────────

fn host_matches(request_host: &str, pattern: &str) -> bool {
    if request_host == pattern {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return request_host.ends_with(suffix) && request_host.len() > suffix.len();
    }
    false
}
