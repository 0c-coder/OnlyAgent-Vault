//! Vault integration — provider-agnostic credential fetching from external vaults,
//! plus OnlyKey hardware-backed secret protection via FIDO2/WebAuthn bridge.
//!
//! The `VaultProvider` trait defines the interface for vault backends (Bitwarden, etc.).
//! `VaultService` is the orchestrator that routes requests to the correct provider.
//!
//! OnlyKey Vault: secrets are encrypted with per-record AES-256-GCM keys. Those record
//! keys are wrapped using a shared secret derived from OnlyKey's `ok.derive_shared_secret()`
//! via the browser FIDO2 bridge. OneCLI stores NO private decryption keys.

pub(crate) mod api;
pub(crate) mod bitwarden;
pub(crate) mod bitwarden_db;

// OnlyKey vault submodules
pub mod cache;
pub mod crypto;
pub mod db;
pub mod models;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sqlx::PgPool;

use crate::db as main_db;

// ── Types ───────────────────────────────────────────────────────────────

/// Provider-agnostic credential returned by any vault provider.
#[derive(Debug)]
pub(crate) struct VaultCredential {
    #[allow(dead_code)]
    pub username: Option<String>,
    pub password: Option<String>,
}

/// Result of a successful pairing operation.
#[derive(Debug)]
pub(crate) struct PairResult {
    /// Human-readable name for the connection (shown in UI).
    pub display_name: Option<String>,
}

/// Connection status for a provider.
#[derive(Debug)]
pub(crate) struct ProviderStatus {
    pub connected: bool,
    pub name: Option<String>,
    /// Provider-specific status details (e.g. fingerprint for Bitwarden).
    /// Serialized as-is into the API response as `status_data`.
    pub status_data: Option<serde_json::Value>,
}

// ── Trait ────────────────────────────────────────────────────────────────

#[async_trait]
pub(crate) trait VaultProvider: Send + Sync {
    /// Provider identifier (e.g., "bitwarden").
    fn provider_name(&self) -> &'static str;

    /// Pair with the vault using provider-specific credentials.
    async fn pair(&self, user_id: &str, params: &serde_json::Value) -> Result<PairResult>;

    /// Request a credential for a hostname from this user's vault.
    async fn request_credential(&self, user_id: &str, hostname: &str) -> Option<VaultCredential>;

    /// Get connection status for this user.
    async fn status(&self, user_id: &str) -> ProviderStatus;

    /// Disconnect and clean up.
    async fn disconnect(&self, user_id: &str) -> Result<()>;
}

// ── Orchestrator ────────────────────────────────────────────────────────

/// Provider-agnostic vault service. Routes operations to the correct provider
/// by name, iterates all providers for credential lookups.
pub(crate) struct VaultService {
    providers: Vec<Box<dyn VaultProvider>>,
    pool: PgPool,
}

impl VaultService {
    pub fn new(providers: Vec<Box<dyn VaultProvider>>, pool: PgPool) -> Self {
        Self { providers, pool }
    }

    /// Try each provider in order until one returns a credential.
    pub async fn request_credential(
        &self,
        user_id: &str,
        hostname: &str,
    ) -> Option<VaultCredential> {
        for provider in &self.providers {
            if let Some(cred) = provider.request_credential(user_id, hostname).await {
                return Some(cred);
            }
        }
        None
    }

    /// Pair with a specific provider. The provider owns DB persistence.
    pub async fn pair(
        &self,
        user_id: &str,
        provider: &str,
        params: &serde_json::Value,
    ) -> Result<PairResult> {
        let p = self.find_provider(provider)?;
        p.pair(user_id, params).await
    }

    /// Get status for a specific provider.
    pub async fn status(&self, user_id: &str, provider: &str) -> Option<ProviderStatus> {
        let p = self.find_provider(provider).ok()?;
        Some(p.status(user_id).await)
    }

    /// Disconnect a specific provider.
    pub async fn disconnect(&self, user_id: &str, provider: &str) -> Result<()> {
        let p = self.find_provider(provider)?;
        p.disconnect(user_id).await?;
        main_db::delete_vault_connection(&self.pool, user_id, provider).await?;
        Ok(())
    }

    fn find_provider(&self, name: &str) -> Result<&dyn VaultProvider> {
        self.providers
            .iter()
            .find(|p| p.provider_name() == name)
            .map(|p| p.as_ref())
            .ok_or_else(|| anyhow!("unknown vault provider: {}", name))
    }
}

// ── GatewayModule implementation ─────────────────────────────────────

use std::future::Future;
use std::pin::Pin;

use axum::Router;
use tracing::info;

use crate::module::GatewayModule;

/// Self-registering vault module for the gateway.
/// Contains both Bitwarden provider-agnostic routes and OnlyKey hardware vault routes.
pub struct VaultModule {
    pub vault_state: Arc<api::VaultState>,
}

impl GatewayModule for VaultModule {
    fn name(&self) -> &'static str {
        "vault"
    }

    fn router(&self) -> Option<(&str, Router)> {
        let router: Router = Router::new()
            .route("/records/{id}/access", axum::routing::post(api::access_record))
            .route("/records/{id}/lock", axum::routing::post(api::lock_record))
            .route("/browser/pending", axum::routing::get(api::get_pending_approvals))
            .route("/browser/approve", axum::routing::post(api::approve_request))
            .route("/agents/{id}/lock", axum::routing::post(api::lock_agent_records))
            .route("/cache/revoke-all", axum::routing::post(api::revoke_all_cache))
            .with_state(Arc::clone(&self.vault_state));

        Some(("/v1/vault", router))
    }

    fn background_tasks(&self) -> Vec<Pin<Box<dyn Future<Output = ()> + Send>>> {
        let vc = self.vault_state.clone();
        let cleanup = async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let removed = vc.unlock_cache.cleanup_expired();
                if removed > 0 {
                    info!(removed = removed, "vault cache: cleaned up expired entries");
                }
            }
        };
        vec![Box::pin(cleanup)]
    }
}
