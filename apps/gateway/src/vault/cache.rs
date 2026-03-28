//! In-memory unlock cache for vault record keys.
//!
//! Stores unwrapped record keys in RAM with absolute TTL and idle timeout.
//! Supports scoped unlocks (per agent/session) and revocation events.
//!
//! SECURITY: Record keys exist ONLY in this cache. They are never persisted.
//! On expiry, revocation, or process restart, all cached keys are lost and
//! must be re-derived via OnlyKey.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tracing::info;

use super::models::{CacheScope, RevocationReason};

// ── Cache entry ─────────────────────────────────────────────────────────

/// A cached unwrapped record key with expiry metadata.
#[derive(Debug, Clone)]
pub struct UnlockCacheEntry {
    pub record_id: String,
    pub scope_type: CacheScope,
    pub scope_id: String,
    pub record_key: Vec<u8>, // raw 32-byte record key
    pub unlocked_at: Instant,
    pub absolute_expires_at: Instant,
    pub idle_expires_at: Instant,
    pub last_used_at: Instant,
    pub policy_version: u32,
    pub key_version: u32,
    pub unlock_generation: u64,
    pub browser_session_id: Option<String>,
}

impl UnlockCacheEntry {
    /// Check if this entry is still valid (not expired by TTL or idle).
    pub fn is_valid(&self) -> bool {
        let now = Instant::now();
        now < self.absolute_expires_at && now < self.idle_expires_at
    }

    /// Touch the entry to reset the idle timeout.
    pub fn touch(&mut self, idle_timeout: Duration) {
        self.last_used_at = Instant::now();
        self.idle_expires_at = self.last_used_at + idle_timeout;
    }
}

// ── Cache key ───────────────────────────────────────────────────────────

/// Composite key for the unlock cache: (record_id, scope_type, scope_id).
fn cache_key(record_id: &str, scope: &CacheScope, scope_id: &str) -> String {
    let scope_str = match scope {
        CacheScope::Global => "global",
        CacheScope::Agent => "agent",
        CacheScope::Session => "session",
    };
    format!("{record_id}:{scope_str}:{scope_id}")
}

// ── InMemoryUnlockCache ─────────────────────────────────────────────────

/// Thread-safe in-memory cache for unlocked vault record keys.
#[derive(Clone)]
pub struct InMemoryUnlockCache {
    inner: Arc<Mutex<HashMap<String, UnlockCacheEntry>>>,
}

impl Default for InMemoryUnlockCache {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryUnlockCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get a valid cache entry, returning None if expired or missing.
    /// Automatically removes expired entries.
    pub fn get(
        &self,
        record_id: &str,
        scope: &CacheScope,
        scope_id: &str,
    ) -> Option<UnlockCacheEntry> {
        let key = cache_key(record_id, scope, scope_id);
        let mut inner = self.inner.lock().ok()?;

        if let Some(entry) = inner.get(&key) {
            if entry.is_valid() {
                return Some(entry.clone());
            }
            // Expired — remove it
            inner.remove(&key);
        }
        None
    }

    /// Get a valid entry and touch it to reset idle timeout.
    pub fn get_and_touch(
        &self,
        record_id: &str,
        scope: &CacheScope,
        scope_id: &str,
        idle_timeout: Duration,
    ) -> Option<UnlockCacheEntry> {
        let key = cache_key(record_id, scope, scope_id);
        let mut inner = self.inner.lock().ok()?;

        if let Some(entry) = inner.get_mut(&key) {
            if entry.is_valid() {
                entry.touch(idle_timeout);
                return Some(entry.clone());
            }
            inner.remove(&key);
        }
        None
    }

    /// Insert or update an unlock cache entry.
    pub fn put(&self, entry: UnlockCacheEntry) {
        let key = cache_key(&entry.record_id, &entry.scope_type, &entry.scope_id);
        if let Ok(mut inner) = self.inner.lock() {
            inner.insert(key, entry);
        }
    }

    /// Revoke (remove) all cache entries for a specific record.
    pub fn revoke_record(&self, record_id: &str, reason: &RevocationReason) {
        if let Ok(mut inner) = self.inner.lock() {
            let before = inner.len();
            inner.retain(|_, entry| entry.record_id != record_id);
            let removed = before - inner.len();
            if removed > 0 {
                info!(
                    record_id = record_id,
                    reason = reason.as_str(),
                    removed = removed,
                    "vault cache: revoked record entries"
                );
            }
        }
    }

    /// Revoke a specific scoped entry for a record.
    pub fn revoke_record_scope(
        &self,
        record_id: &str,
        scope: &CacheScope,
        scope_id: &str,
        reason: &RevocationReason,
    ) {
        let key = cache_key(record_id, scope, scope_id);
        if let Ok(mut inner) = self.inner.lock() {
            if inner.remove(&key).is_some() {
                info!(
                    record_id = record_id,
                    scope_id = scope_id,
                    reason = reason.as_str(),
                    "vault cache: revoked scoped entry"
                );
            }
        }
    }

    /// Revoke all entries for a specific agent (across all records).
    pub fn revoke_agent(&self, agent_id: &str, reason: &RevocationReason) {
        if let Ok(mut inner) = self.inner.lock() {
            let before = inner.len();
            inner.retain(|_, entry| {
                !(entry.scope_type == CacheScope::Agent && entry.scope_id == agent_id)
            });
            let removed = before - inner.len();
            if removed > 0 {
                info!(
                    agent_id = agent_id,
                    reason = reason.as_str(),
                    removed = removed,
                    "vault cache: revoked agent entries"
                );
            }
        }
    }

    /// Revoke all entries for a specific browser session.
    pub fn revoke_browser_session(&self, browser_session_id: &str, reason: &RevocationReason) {
        if let Ok(mut inner) = self.inner.lock() {
            let before = inner.len();
            inner.retain(|_, entry| {
                entry
                    .browser_session_id
                    .as_deref()
                    .map_or(true, |id| id != browser_session_id)
            });
            let removed = before - inner.len();
            if removed > 0 {
                info!(
                    browser_session_id = browser_session_id,
                    reason = reason.as_str(),
                    removed = removed,
                    "vault cache: revoked browser session entries"
                );
            }
        }
    }

    /// Revoke ALL cached entries (e.g., on server restart or admin command).
    pub fn revoke_all(&self, reason: &RevocationReason) {
        if let Ok(mut inner) = self.inner.lock() {
            let count = inner.len();
            inner.clear();
            if count > 0 {
                info!(
                    reason = reason.as_str(),
                    removed = count,
                    "vault cache: revoked all entries"
                );
            }
        }
    }

    /// Run a cleanup pass to remove expired entries. Call periodically.
    pub fn cleanup_expired(&self) -> usize {
        let mut removed = 0;
        if let Ok(mut inner) = self.inner.lock() {
            let before = inner.len();
            inner.retain(|_, entry| entry.is_valid());
            removed = before - inner.len();
        }
        removed
    }

    /// Get the number of cached entries (for monitoring).
    pub fn len(&self) -> usize {
        self.inner.lock().map(|i| i.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ── Helper: should we require fresh unlock? ─────────────────────────────

/// Determine if a fresh OnlyKey unlock is required based on policy and cache state.
pub fn should_require_fresh_unlock(
    require_onlykey: bool,
    require_fresh_for_high_risk: bool,
    cache_hit: bool,
    high_risk: bool,
) -> bool {
    if !require_onlykey {
        return false;
    }
    if high_risk && require_fresh_for_high_risk {
        return true;
    }
    !cache_hit
}

// ── Tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(record_id: &str, scope_id: &str, ttl_secs: u64, idle_secs: u64) -> UnlockCacheEntry {
        let now = Instant::now();
        UnlockCacheEntry {
            record_id: record_id.to_string(),
            scope_type: CacheScope::Agent,
            scope_id: scope_id.to_string(),
            record_key: vec![0u8; 32],
            unlocked_at: now,
            absolute_expires_at: now + Duration::from_secs(ttl_secs),
            idle_expires_at: now + Duration::from_secs(idle_secs),
            last_used_at: now,
            policy_version: 1,
            key_version: 1,
            unlock_generation: 1,
            browser_session_id: Some("bsess_1".to_string()),
        }
    }

    #[test]
    fn cache_put_and_get() {
        let cache = InMemoryUnlockCache::new();
        let entry = make_entry("rec_1", "agent_1", 3600, 600);
        cache.put(entry.clone());

        let result = cache.get("rec_1", &CacheScope::Agent, "agent_1");
        assert!(result.is_some());
        assert_eq!(result.unwrap().record_id, "rec_1");
    }

    #[test]
    fn cache_miss_returns_none() {
        let cache = InMemoryUnlockCache::new();
        assert!(cache.get("rec_1", &CacheScope::Agent, "agent_1").is_none());
    }

    #[test]
    fn cache_expired_entry_removed() {
        let cache = InMemoryUnlockCache::new();
        let now = Instant::now();
        let entry = UnlockCacheEntry {
            record_id: "rec_1".to_string(),
            scope_type: CacheScope::Agent,
            scope_id: "agent_1".to_string(),
            record_key: vec![0u8; 32],
            unlocked_at: now - Duration::from_secs(100),
            absolute_expires_at: now - Duration::from_secs(1), // already expired
            idle_expires_at: now + Duration::from_secs(600),
            last_used_at: now,
            policy_version: 1,
            key_version: 1,
            unlock_generation: 1,
            browser_session_id: None,
        };
        cache.put(entry);
        assert!(cache.get("rec_1", &CacheScope::Agent, "agent_1").is_none());
    }

    #[test]
    fn revoke_record_removes_all_scopes() {
        let cache = InMemoryUnlockCache::new();
        cache.put(make_entry("rec_1", "agent_1", 3600, 600));
        cache.put(make_entry("rec_1", "agent_2", 3600, 600));
        cache.put(make_entry("rec_2", "agent_1", 3600, 600));

        cache.revoke_record("rec_1", &RevocationReason::ManualRevoke);

        assert!(cache.get("rec_1", &CacheScope::Agent, "agent_1").is_none());
        assert!(cache.get("rec_1", &CacheScope::Agent, "agent_2").is_none());
        assert!(cache.get("rec_2", &CacheScope::Agent, "agent_1").is_some());
    }

    #[test]
    fn revoke_agent_removes_agent_entries() {
        let cache = InMemoryUnlockCache::new();
        cache.put(make_entry("rec_1", "agent_1", 3600, 600));
        cache.put(make_entry("rec_2", "agent_1", 3600, 600));
        cache.put(make_entry("rec_3", "agent_2", 3600, 600));

        cache.revoke_agent("agent_1", &RevocationReason::AdminRevoke);

        assert!(cache.get("rec_1", &CacheScope::Agent, "agent_1").is_none());
        assert!(cache.get("rec_2", &CacheScope::Agent, "agent_1").is_none());
        assert!(cache.get("rec_3", &CacheScope::Agent, "agent_2").is_some());
    }

    #[test]
    fn revoke_all_clears_everything() {
        let cache = InMemoryUnlockCache::new();
        cache.put(make_entry("rec_1", "agent_1", 3600, 600));
        cache.put(make_entry("rec_2", "agent_2", 3600, 600));

        cache.revoke_all(&RevocationReason::ServerRestart);

        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn revoke_browser_session() {
        let cache = InMemoryUnlockCache::new();
        cache.put(make_entry("rec_1", "agent_1", 3600, 600)); // bsess_1
        let mut entry2 = make_entry("rec_2", "agent_1", 3600, 600);
        entry2.browser_session_id = Some("bsess_2".to_string());
        cache.put(entry2);

        cache.revoke_browser_session("bsess_1", &RevocationReason::BrowserDisconnect);

        assert!(cache.get("rec_1", &CacheScope::Agent, "agent_1").is_none());
        assert!(cache.get("rec_2", &CacheScope::Agent, "agent_1").is_some());
    }

    #[test]
    fn should_require_fresh_unlock_logic() {
        // Not required when onlykey not required
        assert!(!should_require_fresh_unlock(false, true, false, true));

        // Required when high risk and policy says so
        assert!(should_require_fresh_unlock(true, true, true, true));

        // Required when no cache hit
        assert!(should_require_fresh_unlock(true, false, false, false));

        // Not required when cache hit and not high risk
        assert!(!should_require_fresh_unlock(true, true, true, false));
    }

    #[test]
    fn cleanup_expired_removes_stale() {
        let cache = InMemoryUnlockCache::new();
        cache.put(make_entry("rec_1", "agent_1", 3600, 600)); // valid
        let now = Instant::now();
        let expired = UnlockCacheEntry {
            record_id: "rec_2".to_string(),
            scope_type: CacheScope::Agent,
            scope_id: "agent_1".to_string(),
            record_key: vec![0u8; 32],
            unlocked_at: now - Duration::from_secs(100),
            absolute_expires_at: now - Duration::from_secs(1),
            idle_expires_at: now - Duration::from_secs(1),
            last_used_at: now - Duration::from_secs(50),
            policy_version: 1,
            key_version: 1,
            unlock_generation: 1,
            browser_session_id: None,
        };
        cache.put(expired);

        let removed = cache.cleanup_expired();
        assert_eq!(removed, 1);
        assert_eq!(cache.len(), 1);
    }
}
