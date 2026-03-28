//! Data models for the OnlyKey Vault system.

use serde::{Deserialize, Serialize};

// ── Record types ────────────────────────────────────────────────────────

/// Type of secret stored in a vault record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RecordType {
    ApiKey,
    OauthToken,
    AgeSecret,
    GenericSecret,
}

impl RecordType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ApiKey => "api_key",
            Self::OauthToken => "oauth_token",
            Self::AgeSecret => "age_secret",
            Self::GenericSecret => "generic_secret",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "api_key" => Some(Self::ApiKey),
            "oauth_token" => Some(Self::OauthToken),
            "age_secret" => Some(Self::AgeSecret),
            "generic_secret" => Some(Self::GenericSecret),
            _ => None,
        }
    }
}

// ── Cache scope ─────────────────────────────────────────────────────────

/// Scope for unlock cache entries.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum CacheScope {
    Global,
    Agent,
    Session,
}

impl CacheScope {
    pub fn from_str(s: &str) -> Self {
        match s {
            "global" => Self::Global,
            "session" => Self::Session,
            _ => Self::Agent, // default
        }
    }
}

// ── Approval status ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalStatus {
    Pending,
    Approved,
    Denied,
    Expired,
}

// ── Browser operation ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BrowserOperation {
    UnlockRecordKey,
    DecryptAge,
    SignBlob,
}

// ── Unlock reason ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UnlockReason {
    OnlykeyApproved,
    AdminOverride,
}

// ── Revocation reason ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum RevocationReason {
    ManualRevoke,
    TtlExpired,
    IdleTimeout,
    BrowserDisconnect,
    PolicyChanged,
    KeyRotated,
    ServerRestart,
    AdminRevoke,
}

impl RevocationReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ManualRevoke => "manual_revoke",
            Self::TtlExpired => "ttl_expired",
            Self::IdleTimeout => "idle_timeout",
            Self::BrowserDisconnect => "browser_disconnect",
            Self::PolicyChanged => "policy_changed",
            Self::KeyRotated => "key_rotated",
            Self::ServerRestart => "server_restart",
            Self::AdminRevoke => "admin_revoke",
        }
    }
}

// ── Record policy (parsed from DB) ──────────────────────────────────────

/// Policy governing how a vault record can be unlocked and cached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordPolicy {
    pub require_onlykey: bool,
    pub unlock_ttl_seconds: u64,
    pub idle_timeout_seconds: u64,
    pub cache_scope: CacheScope,
    pub allow_manual_revoke: bool,
    pub relock_on_browser_disconnect: bool,
    pub relock_on_policy_change: bool,
    pub require_fresh_unlock_for_high_risk: bool,
    pub allow_plaintext_return: bool,
    pub allowed_agents: Vec<String>,
}

// ── Derivation context ──────────────────────────────────────────────────

/// Context passed to ok.derive_shared_secret as AdditionalData.
/// Must be stable and deterministic for the same record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DerivationContext {
    pub record_id: String,
    pub purpose: String,
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tenant_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
}

// ── API request/response types ──────────────────────────────────────────

/// Agent's request to access a vault record.
#[derive(Debug, Deserialize)]
pub struct AccessRecordRequest {
    pub agent_id: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub high_risk: bool,
    #[serde(default)]
    pub purpose: Option<String>,
}

/// Response to an access request.
#[derive(Debug, Serialize)]
pub struct AccessRecordResponse {
    pub status: String, // "ok", "pending_approval", "denied"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// Browser's approval payload after OnlyKey derivation.
#[derive(Debug, Deserialize)]
pub struct BrowserApprovePayload {
    pub request_id: String,
    pub derived_secret_b64: String,
    pub browser_session_id: String,
}

/// Response to browser approval.
#[derive(Debug, Serialize)]
pub struct BrowserApproveResponse {
    pub status: String, // "ok", "error"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Pending approval as seen by the browser.
#[derive(Debug, Serialize)]
pub struct PendingApprovalView {
    pub request_id: String,
    pub record_id: String,
    pub record_name: String,
    pub agent_id: String,
    pub operation: String,
    pub origin: String,
    pub onecli_record_pubkey_jwk: serde_json::Value,
    pub additional_data: serde_json::Value,
    pub created_at: String,
    pub expires_at: String,
    pub nonce_b64: String,
}

/// Audit event (for logging).
#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub event: String,
    pub record_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    pub timestamp: String,
}
