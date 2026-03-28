//! OnlyAgent Hands — remote machine control via OnlyKey WebHID.
//!
//! This module handles:
//! - Keystroke instruction compilation (macro → OS-specific HID keystrokes)
//! - HID packet framing (CBOR → chunked reports)
//! - Session lifecycle management
//! - Job/step orchestration
//! - Screenshot storage and retrieval
//! - Audit logging

pub mod api;
pub mod compile;
pub mod db;
pub mod models;
pub mod packet;
pub mod session;

// ── GatewayModule implementation ─────────────────────────────────────

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use axum::Router;
use tracing::info;

use crate::module::GatewayModule;

/// Self-registering Hands module for the gateway.
pub struct HandsModule {
    pub hands_state: Arc<api::HandsState>,
    pub session_manager: Arc<session::SessionManager>,
}

impl GatewayModule for HandsModule {
    fn name(&self) -> &'static str {
        "hands"
    }

    fn router(&self) -> Option<(&str, Router)> {
        let router: Router = Router::new()
            .route("/jobs", axum::routing::post(api::create_job))
            .route("/jobs", axum::routing::get(api::list_jobs))
            .route("/jobs/{id}", axum::routing::get(api::get_job))
            .route("/jobs/{id}/start", axum::routing::post(api::start_job))
            .route("/jobs/{id}/cancel", axum::routing::post(api::cancel_job))
            .route("/sessions", axum::routing::post(api::create_session))
            .route("/sessions/{id}", axum::routing::get(api::get_session))
            .route("/sessions/{id}", axum::routing::delete(api::close_session))
            .route("/sessions/{id}/activated", axum::routing::post(api::activate_session))
            .route("/sessions/{id}/emergency-stop", axum::routing::post(api::emergency_stop))
            .route("/sessions/{id}/next-packet", axum::routing::get(api::next_packet))
            .route("/sessions/{id}/packet-acked", axum::routing::post(api::packet_acked))
            .route("/sessions/{id}/step-status", axum::routing::post(api::step_status))
            .route("/screenshots", axum::routing::post(api::upload_screenshot))
            .route("/screenshots/{id}", axum::routing::get(api::get_screenshot))
            .with_state(Arc::clone(&self.hands_state));

        Some(("/v1/hands", router))
    }

    fn background_tasks(&self) -> Vec<Pin<Box<dyn Future<Output = ()> + Send>>> {
        let sm = self.session_manager.clone();
        let cleanup = async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
            loop {
                interval.tick().await;
                let stale = sm.cleanup_stale(300);
                if stale > 0 {
                    info!(stale = stale, "hands: cleaned up stale sessions");
                }
            }
        };
        vec![Box::pin(cleanup)]
    }
}
