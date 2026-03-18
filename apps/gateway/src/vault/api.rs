//! Axum handlers for vault operations.
//!
//! Bitwarden vault (provider-agnostic):
//!   POST   /api/vault/:provider/pair     — Pair with a vault provider
//!   GET    /api/vault/:provider/status   — Get connection status
//!   DELETE /api/vault/:provider/pair     — Disconnect a provider
//!
//! OnlyKey vault (hardware-backed):
//!   POST /v1/vault/records/:id/access   — Request access to a vault secret
//!   POST /v1/vault/records/:id/lock     — Manually lock a record
//!   GET  /v1/vault/browser/pending      — Poll for pending approval requests
//!   POST /v1/vault/browser/approve      — Submit OnlyKey-derived approval
//!   POST /v1/vault/agents/:id/lock      — Lock all records for an agent
//!   POST /v1/vault/cache/revoke-all     — Revoke all cached keys

use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use base64::Engine;
use ring::rand::{SecureRandom, SystemRandom};
use tracing::{info, warn};

use crate::auth::AuthUser;
use crate::gateway::GatewayState;

use super::cache::{should_require_fresh_unlock, InMemoryUnlockCache, UnlockCacheEntry};
use super::crypto;
use super::db as vault_db;
use super::models::*;

// ── OnlyKey Vault state shared across handlers ──────────────────────────

/// Shared state for OnlyKey vault API handlers.
#[derive(Clone)]
pub struct VaultState {
    pub pool: sqlx::PgPool,
    pub unlock_cache: InMemoryUnlockCache,
    pub default_origin: String,
}

// ── Bitwarden vault endpoints (provider-agnostic) ───────────────────────

/// POST /api/vault/:provider/pair
/// Body: provider-specific JSON (e.g. `{ psk_hex, fingerprint_hex }` for Bitwarden)
pub(crate) async fn vault_pair(
    auth: AuthUser,
    State(state): State<GatewayState>,
    Path(provider): Path<String>,
    Json(params): Json<serde_json::Value>,
) -> impl IntoResponse {
    match state
        .vault_service
        .pair(&auth.user_id, &provider, &params)
        .await
    {
        Ok(result) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "paired",
                "display_name": result.display_name,
            })),
        ),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// GET /api/vault/:provider/status
pub(crate) async fn vault_status(
    auth: AuthUser,
    State(state): State<GatewayState>,
    Path(provider): Path<String>,
) -> impl IntoResponse {
    match state.vault_service.status(&auth.user_id, &provider).await {
        Some(status) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "connected": status.connected,
                "name": status.name,
                "status_data": status.status_data,
            })),
        ),
        None => (
            StatusCode::OK,
            Json(serde_json::json!({
                "connected": false,
                "name": null,
                "status_data": null,
            })),
        ),
    }
}

/// DELETE /api/vault/:provider/pair
pub(crate) async fn vault_disconnect(
    auth: AuthUser,
    State(state): State<GatewayState>,
    Path(provider): Path<String>,
) -> impl IntoResponse {
    match state
        .vault_service
        .disconnect(&auth.user_id, &provider)
        .await
    {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "disconnected"})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

// ── OnlyKey vault: Agent endpoints ──────────────────────────────────────

/// POST /v1/vault/records/:id/access
///
/// Agent requests access to a vault-protected secret.
/// Returns the secret if already unlocked, or creates a pending approval.
pub async fn access_record(
    State(state): State<Arc<VaultState>>,
    Path(record_id): Path<String>,
    Json(req): Json<AccessRecordRequest>,
) -> impl IntoResponse {
    // 1. Load the record
    let record = match vault_db::find_vault_record(&state.pool, &record_id).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(AccessRecordResponse {
                    status: "denied".to_string(),
                    request_id: None,
                    secret: None,
                    expires_at: None,
                }),
            );
        }
        Err(e) => {
            warn!(error = %e, "vault access: db error");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AccessRecordResponse {
                    status: "denied".to_string(),
                    request_id: None,
                    secret: None,
                    expires_at: None,
                }),
            );
        }
    };

    // 2. Check if agent is allowed
    if let Some(ref allowed) = record.allowed_agents {
        if let Ok(agents) = serde_json::from_str::<Vec<String>>(allowed) {
            if !agents.is_empty() && !agents.contains(&req.agent_id) {
                return (
                    StatusCode::FORBIDDEN,
                    Json(AccessRecordResponse {
                        status: "denied".to_string(),
                        request_id: None,
                        secret: None,
                        expires_at: None,
                    }),
                );
            }
        }
    }

    // 3. Determine scope
    let scope = CacheScope::from_str(&record.cache_scope);
    let scope_id = match scope {
        CacheScope::Session => req.session_id.as_deref().unwrap_or(&req.agent_id),
        _ => &req.agent_id,
    };
    let idle_timeout = Duration::from_secs(record.idle_timeout_seconds as u64);

    // 4. Check unlock cache
    let cache_entry = state
        .unlock_cache
        .get_and_touch(&record_id, &scope, scope_id, idle_timeout);

    let cache_hit = cache_entry.is_some();
    let needs_fresh = should_require_fresh_unlock(
        record.require_onlykey,
        record.require_fresh_unlock_for_high_risk,
        cache_hit,
        req.high_risk,
    );

    if needs_fresh {
        // Create pending approval
        let rng = SystemRandom::new();
        let mut nonce = [0u8; 32];
        let _ = rng.fill(&mut nonce);
        let nonce_b64 = base64::engine::general_purpose::STANDARD.encode(nonce);

        let request_id = generate_id("vreq");
        let expires_at =
            chrono::Utc::now().naive_utc() + chrono::Duration::seconds(300); // 5 min

        if let Err(e) = vault_db::create_approval_request(
            &state.pool,
            &request_id,
            &record_id,
            &req.agent_id,
            req.session_id.as_deref(),
            "unlock_record_key",
            &state.default_origin,
            &nonce_b64,
            expires_at,
        )
        .await
        {
            warn!(error = %e, "vault access: failed to create approval");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AccessRecordResponse {
                    status: "denied".to_string(),
                    request_id: None,
                    secret: None,
                    expires_at: None,
                }),
            );
        }

        // Log audit event
        let _ = vault_db::insert_audit_event(
            &state.pool,
            &generate_id("vaud"),
            &record_id,
            "unlock_requested",
            Some(&req.agent_id),
            req.session_id.as_deref(),
            None,
            None,
            None,
            None,
        )
        .await;

        info!(
            record_id = record_id,
            agent_id = req.agent_id,
            request_id = request_id,
            "vault: created pending approval"
        );

        return (
            StatusCode::ACCEPTED,
            Json(AccessRecordResponse {
                status: "pending_approval".to_string(),
                request_id: Some(request_id),
                secret: None,
                expires_at: Some(expires_at.to_string()),
            }),
        );
    }

    // 5. Use cached record key to decrypt
    if let Some(entry) = cache_entry {
        match crypto::decrypt_vault_secret(
            &entry.record_key,
            &record.ciphertext_b64,
            &record.nonce_b64,
            &record.aad_json,
        ) {
            Ok(plaintext) => {
                // Log usage
                let _ = vault_db::insert_audit_event(
                    &state.pool,
                    &generate_id("vaud"),
                    &record_id,
                    "key_used",
                    Some(&req.agent_id),
                    req.session_id.as_deref(),
                    Some(&record.cache_scope),
                    Some(scope_id),
                    None,
                    None,
                )
                .await;

                return (
                    StatusCode::OK,
                    Json(AccessRecordResponse {
                        status: "ok".to_string(),
                        request_id: None,
                        secret: if record.allow_plaintext_return {
                            Some(plaintext)
                        } else {
                            None
                        },
                        expires_at: None,
                    }),
                );
            }
            Err(e) => {
                warn!(error = %e, record_id = record_id, "vault: decrypt failed with cached key");
                // Invalidate bad cache entry
                state.unlock_cache.revoke_record(
                    &record_id,
                    &RevocationReason::PolicyChanged,
                );
            }
        }
    }

    // Fallback: should not reach here normally
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(AccessRecordResponse {
            status: "denied".to_string(),
            request_id: None,
            secret: None,
            expires_at: None,
        }),
    )
}

/// POST /v1/vault/records/:id/lock
///
/// Manually lock a vault record (revoke cached key).
pub async fn lock_record(
    State(state): State<Arc<VaultState>>,
    Path(record_id): Path<String>,
) -> impl IntoResponse {
    state
        .unlock_cache
        .revoke_record(&record_id, &RevocationReason::ManualRevoke);

    let _ = vault_db::insert_audit_event(
        &state.pool,
        &generate_id("vaud"),
        &record_id,
        "key_revoked",
        None,
        None,
        None,
        None,
        Some("manual_revoke"),
        None,
    )
    .await;

    StatusCode::OK
}

// ── OnlyKey vault: Browser endpoints ────────────────────────────────────

/// GET /v1/vault/browser/pending?user_id=...
///
/// Browser polls for pending approval requests.
pub async fn get_pending_approvals(
    State(state): State<Arc<VaultState>>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let user_id = match params.get("user_id") {
        Some(id) => id,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "user_id required"})),
            );
        }
    };

    match vault_db::find_pending_approvals(&state.pool, user_id).await {
        Ok(items) => {
            let views: Vec<PendingApprovalView> = items
                .into_iter()
                .map(|(approval, record)| {
                    let pubkey_jwk: serde_json::Value =
                        serde_json::from_str(&record.onecli_pubkey_jwk).unwrap_or_default();
                    let additional_data: serde_json::Value =
                        serde_json::from_str(&record.derivation_context).unwrap_or_default();

                    PendingApprovalView {
                        request_id: approval.id,
                        record_id: approval.record_id,
                        record_name: record.name,
                        agent_id: approval.agent_id,
                        operation: approval.operation,
                        origin: approval.origin,
                        onecli_record_pubkey_jwk: pubkey_jwk,
                        additional_data,
                        created_at: approval.created_at.to_string(),
                        expires_at: approval.expires_at.to_string(),
                        nonce_b64: approval.nonce_b64,
                    }
                })
                .collect();

            (StatusCode::OK, Json(serde_json::json!({ "items": views })))
        }
        Err(e) => {
            warn!(error = %e, "vault browser: failed to fetch pending");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "internal error"})),
            )
        }
    }
}

/// POST /v1/vault/browser/approve
///
/// Browser submits OnlyKey-derived shared secret to approve a pending request.
/// Gateway uses it to unwrap record key, caches key in memory, completes the approval.
pub async fn approve_request(
    State(state): State<Arc<VaultState>>,
    Json(payload): Json<BrowserApprovePayload>,
) -> impl IntoResponse {
    // 1. Load the approval request
    let approval = match vault_db::find_approval_by_id(&state.pool, &payload.request_id).await {
        Ok(Some(a)) if a.status == "pending" => a,
        Ok(Some(_)) => {
            return (
                StatusCode::CONFLICT,
                Json(BrowserApproveResponse {
                    status: "error".to_string(),
                    message: Some("request already processed".to_string()),
                }),
            );
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(BrowserApproveResponse {
                    status: "error".to_string(),
                    message: Some("request not found".to_string()),
                }),
            );
        }
        Err(e) => {
            warn!(error = %e, "vault approve: db error");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(BrowserApproveResponse {
                    status: "error".to_string(),
                    message: Some("internal error".to_string()),
                }),
            );
        }
    };

    // Check expiry
    if approval.expires_at < chrono::Utc::now().naive_utc() {
        let _ =
            vault_db::update_approval_status(&state.pool, &payload.request_id, "expired", None)
                .await;
        return (
            StatusCode::GONE,
            Json(BrowserApproveResponse {
                status: "error".to_string(),
                message: Some("request expired".to_string()),
            }),
        );
    }

    // 2. Load the vault record
    let record = match vault_db::find_vault_record(&state.pool, &approval.record_id).await {
        Ok(Some(r)) => r,
        _ => {
            return (
                StatusCode::NOT_FOUND,
                Json(BrowserApproveResponse {
                    status: "error".to_string(),
                    message: Some("record not found".to_string()),
                }),
            );
        }
    };

    // 3. Unwrap the record key using the browser-derived shared secret
    let record_key = match crypto::unwrap_record_key(
        &payload.derived_secret_b64,
        &record.wrapped_key_b64,
        &record.wrapped_key_nonce_b64,
        &record.derivation_context,
    ) {
        Ok(key) => key,
        Err(e) => {
            warn!(error = %e, record_id = record.id, "vault approve: unwrap failed");
            let _ = vault_db::update_approval_status(
                &state.pool,
                &payload.request_id,
                "denied",
                Some(&payload.browser_session_id),
            )
            .await;
            return (
                StatusCode::UNAUTHORIZED,
                Json(BrowserApproveResponse {
                    status: "error".to_string(),
                    message: Some("key unwrap failed — wrong OnlyKey or derivation mismatch".to_string()),
                }),
            );
        }
    };

    // 4. Cache the record key in memory
    let now = Instant::now();
    let ttl = Duration::from_secs(record.unlock_ttl_seconds as u64);
    let idle = Duration::from_secs(record.idle_timeout_seconds as u64);
    let scope = CacheScope::from_str(&record.cache_scope);
    let scope_id = match scope {
        CacheScope::Session => approval
            .session_id
            .as_deref()
            .unwrap_or(&approval.agent_id),
        _ => &approval.agent_id,
    };

    let cache_entry = UnlockCacheEntry {
        record_id: record.id.clone(),
        scope_type: scope.clone(),
        scope_id: scope_id.to_string(),
        record_key,
        unlocked_at: now,
        absolute_expires_at: now + ttl,
        idle_expires_at: now + idle,
        last_used_at: now,
        policy_version: record.policy_version as u32,
        key_version: record.key_version as u32,
        unlock_generation: record.unlock_generation as u64,
        browser_session_id: Some(payload.browser_session_id.clone()),
    };

    state.unlock_cache.put(cache_entry);

    // 5. Update approval status
    let _ = vault_db::update_approval_status(
        &state.pool,
        &payload.request_id,
        "approved",
        Some(&payload.browser_session_id),
    )
    .await;

    // 6. Audit
    let _ = vault_db::insert_audit_event(
        &state.pool,
        &generate_id("vaud"),
        &record.id,
        "unlock_approved",
        Some(&approval.agent_id),
        approval.session_id.as_deref(),
        Some(&record.cache_scope),
        Some(scope_id),
        Some("onlykey_approved"),
        None,
    )
    .await;

    info!(
        record_id = record.id,
        agent_id = approval.agent_id,
        request_id = payload.request_id,
        ttl_seconds = record.unlock_ttl_seconds,
        "vault: approval completed, key cached"
    );

    (
        StatusCode::OK,
        Json(BrowserApproveResponse {
            status: "ok".to_string(),
            message: None,
        }),
    )
}

// ── OnlyKey vault: Admin endpoints ──────────────────────────────────────

/// POST /v1/vault/agents/:id/lock
pub async fn lock_agent_records(
    State(state): State<Arc<VaultState>>,
    Path(agent_id): Path<String>,
) -> impl IntoResponse {
    state
        .unlock_cache
        .revoke_agent(&agent_id, &RevocationReason::AdminRevoke);
    StatusCode::OK
}

/// POST /v1/vault/cache/revoke-all
pub async fn revoke_all_cache(State(state): State<Arc<VaultState>>) -> impl IntoResponse {
    state
        .unlock_cache
        .revoke_all(&RevocationReason::AdminRevoke);
    StatusCode::OK
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn generate_id(prefix: &str) -> String {
    format!("{}_{}", prefix, ulid::Ulid::new().to_string().to_lowercase())
}
