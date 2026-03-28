//! Gateway module trait — self-registering feature modules.
//!
//! Each feature (vault, hands, etc.) implements `GatewayModule` so it can
//! register its own routes, state, and background tasks without editing
//! the central gateway.rs or main.rs files.
//!
//! To add a new feature module:
//! 1. Create `src/<feature>/mod.rs` implementing `GatewayModule`
//! 2. Add `mod <feature>;` to main.rs
//! 3. Call `<feature>::module(pool, ...)` in the modules vec in main.rs
//!
//! That's it — no edits to gateway.rs, GatewayState, or GatewayServer::new().

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;

/// A self-contained gateway feature module.
///
/// Each module provides:
/// - A name (for logging)
/// - An optional Axum sub-router mounted at a given path
/// - Zero or more background tasks (cleanup loops, watchers, etc.)
pub trait GatewayModule: Send + Sync + 'static {
    /// Human-readable module name (e.g., "vault", "hands").
    fn name(&self) -> &'static str;

    /// Optional sub-router to nest into the main Axum router.
    /// Returns `(mount_path, router)` — e.g., `("/v1/vault", router)`.
    /// Return `None` if the module has no HTTP routes.
    fn router(&self) -> Option<(&str, Router)>;

    /// Background tasks to spawn at startup (e.g., periodic cleanup).
    /// Each future runs forever (or until the process exits).
    fn background_tasks(&self) -> Vec<Pin<Box<dyn Future<Output = ()> + Send>>>;
}

/// Collection of registered gateway modules.
/// Used by `GatewayServer` to auto-mount routes and spawn tasks.
pub struct ModuleRegistry {
    modules: Vec<Box<dyn GatewayModule>>,
}

impl ModuleRegistry {
    pub fn new() -> Self {
        Self { modules: Vec::new() }
    }

    /// Register a module. Order doesn't matter — routes are mounted by path.
    pub fn register(&mut self, module: impl GatewayModule) {
        self.modules.push(Box::new(module));
    }

    /// Consume the registry, returning all modules.
    pub fn into_modules(self) -> Vec<Box<dyn GatewayModule>> {
        self.modules
    }
}
