//! HTTP API endpoints for OnlyAgent Hands.
//!
//! Session management:
//!   POST   /v1/hands/sessions                    — Create control session
//!   GET    /v1/hands/sessions/:id                — Get session status
//!   DELETE /v1/hands/sessions/:id                — Close session
//!   POST   /v1/hands/sessions/:id/activated      — Browser reports OnlyKey confirmed
//!   POST   /v1/hands/sessions/:id/emergency-stop — Force stop
//!
//! Jobs:
//!   POST   /v1/hands/jobs        — Create job
//!   GET    /v1/hands/jobs        — List jobs
//!   GET    /v1/hands/jobs/:id    — Get job + steps
//!   POST   /v1/hands/jobs/:id/start  — Start job
//!   POST   /v1/hands/jobs/:id/cancel — Cancel job
//!
//! Instruction delivery (browser ↔ gateway):
//!   GET    /v1/hands/sessions/:id/next-packet    — Get next compiled packet
//!   POST   /v1/hands/sessions/:id/packet-acked   — Browser confirms delivery
//!   POST   /v1/hands/sessions/:id/step-status    — Browser forwards device status
//!
//! Screenshots:
//!   POST   /v1/hands/screenshots     — Upload screenshot
//!   GET    /v1/hands/screenshots/:id  — Retrieve screenshot

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use base64::Engine;
use tracing::{info, warn};

use super::compile;
use super::db as hands_db;
use super::models::*;
use super::packet;
use super::session::{QueuedPacket, SessionManager};

// ── Hands state shared across all handlers ─────────────────────────────

#[derive(Clone)]
pub struct HandsState {
    pub pool: sqlx::PgPool,
    pub session_manager: Arc<SessionManager>,
}

// ── Job endpoints ──────────────────────────────────────────────────────

/// POST /v1/hands/jobs
pub async fn create_job(
    State(state): State<Arc<HandsState>>,
    Json(req): Json<CreateJobRequest>,
) -> impl IntoResponse {
    let job_id = generate_id("hj");

    if let Err(e) = hands_db::create_job(
        &state.pool,
        &job_id,
        "current-user", // TODO: extract from auth
        &req.name,
        &req.description,
        req.host_os.as_deref(),
    )
    .await
    {
        warn!(error = %e, "hands: failed to create job");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to create job"})),
        );
    }

    // Create steps
    for (i, step) in req.steps.iter().enumerate() {
        let step_id = generate_id("hs");
        if let Err(e) = hands_db::create_step(
            &state.pool,
            &step_id,
            &job_id,
            i as i32,
            &step.description,
            &step.macro_instructions,
            &step.expected_outcome,
            step.max_retries,
            step.timeout_ms,
            step.require_confirm,
        )
        .await
        {
            warn!(error = %e, "hands: failed to create step");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "failed to create step"})),
            );
        }
    }

    let _ = hands_db::insert_audit_event(
        &state.pool,
        &generate_id("ha"),
        &job_id,
        "job_created",
        None,
        None,
        None,
    )
    .await;

    info!(job_id = job_id, steps = req.steps.len(), "hands: job created");

    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": job_id, "status": "draft" })),
    )
}

/// GET /v1/hands/jobs
pub async fn list_jobs(
    State(state): State<Arc<HandsState>>,
) -> impl IntoResponse {
    let user_id = "current-user"; // TODO: extract from auth
    match hands_db::find_jobs_by_user(&state.pool, user_id).await {
        Ok(jobs) => {
            let mut summaries = Vec::new();
            for job in jobs {
                let step_count = hands_db::count_steps(&state.pool, &job.id)
                    .await
                    .unwrap_or(0);
                summaries.push(JobSummary {
                    id: job.id,
                    name: job.name,
                    description: job.description,
                    status: job.status,
                    host_os: job.host_os,
                    step_count,
                    created_at: job.created_at.to_string(),
                });
            }
            (StatusCode::OK, Json(serde_json::json!({ "items": summaries })))
        }
        Err(e) => {
            warn!(error = %e, "hands: failed to list jobs");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "internal error"})),
            )
        }
    }
}

/// GET /v1/hands/jobs/:id
pub async fn get_job(
    State(state): State<Arc<HandsState>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let job = match hands_db::find_job(&state.pool, &job_id).await {
        Ok(Some(j)) => j,
        Ok(None) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"})));
        }
        Err(e) => {
            warn!(error = %e, "hands: db error");
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "internal"})));
        }
    };

    let steps = hands_db::find_steps_by_job(&state.pool, &job_id)
        .await
        .unwrap_or_default();

    let step_views: Vec<StepView> = steps
        .into_iter()
        .map(|s| StepView {
            id: s.id,
            sequence_number: s.sequence_number,
            description: s.description,
            macro_instructions: s.macro_instructions,
            expected_outcome: s.expected_outcome,
            status: s.status,
            retry_count: s.retry_count,
            max_retries: s.max_retries,
            timeout_ms: s.timeout_ms,
            require_confirm: s.require_confirm,
            error_message: s.error_message,
        })
        .collect();

    let detail = JobDetail {
        id: job.id,
        name: job.name,
        description: job.description,
        status: job.status,
        host_os: job.host_os,
        max_duration_secs: job.max_duration_secs,
        steps: step_views,
        created_at: job.created_at.to_string(),
        updated_at: job.updated_at.to_string(),
        completed_at: job.completed_at.map(|t| t.to_string()),
    };

    (StatusCode::OK, Json(serde_json::to_value(detail).unwrap()))
}

/// POST /v1/hands/jobs/:id/start
pub async fn start_job(
    State(state): State<Arc<HandsState>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    if let Err(e) = hands_db::update_job_status(&state.pool, &job_id, "queued").await {
        warn!(error = %e, "hands: failed to start job");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    let _ = hands_db::insert_audit_event(
        &state.pool, &generate_id("ha"), &job_id, "job_started", None, None, None,
    ).await;
    info!(job_id = job_id, "hands: job queued for execution");
    StatusCode::OK
}

/// POST /v1/hands/jobs/:id/cancel
pub async fn cancel_job(
    State(state): State<Arc<HandsState>>,
    Path(job_id): Path<String>,
) -> impl IntoResponse {
    let _ = hands_db::update_job_status(&state.pool, &job_id, "cancelled").await;
    let _ = hands_db::insert_audit_event(
        &state.pool, &generate_id("ha"), &job_id, "job_cancelled", None, None, None,
    ).await;
    StatusCode::OK
}

// ── Session endpoints ──────────────────────────────────────────────────

/// POST /v1/hands/sessions
pub async fn create_session(
    State(state): State<Arc<HandsState>>,
    Json(req): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let session_id = generate_id("hses");
    let agent_token = generate_id("hsat");

    // Generate nonce for WebHID session auth
    let mut nonce = [0u8; 16];
    for byte in nonce.iter_mut() {
        *byte = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .subsec_nanos()
            & 0xFF) as u8;
    }
    let nonce_b64 = base64::engine::general_purpose::STANDARD.encode(nonce);

    if let Err(e) = hands_db::create_session(
        &state.pool,
        &session_id,
        &req.job_id,
        "current-user", // TODO: extract from auth
        &agent_token,
        req.host_os.as_deref(),
    )
    .await
    {
        warn!(error = %e, "hands: failed to create session");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to create session"})),
        );
    }

    // Register in-memory session
    state.session_manager.register(session_id.clone(), req.job_id.clone());

    let _ = hands_db::insert_audit_event(
        &state.pool, &generate_id("ha"), &req.job_id,
        "session_created", None, Some(&session_id), None,
    ).await;

    info!(session_id = session_id, job_id = req.job_id, "hands: session created");

    (
        StatusCode::CREATED,
        Json(serde_json::to_value(CreateSessionResponse {
            session_id,
            nonce: nonce_b64,
            agent_token,
        })
        .unwrap()),
    )
}

/// GET /v1/hands/sessions/:id
pub async fn get_session(
    State(state): State<Arc<HandsState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    match hands_db::find_session(&state.pool, &session_id).await {
        Ok(Some(s)) => (
            StatusCode::OK,
            Json(serde_json::to_value(SessionView {
                id: s.id,
                job_id: s.job_id,
                status: s.status,
                host_os: s.host_os,
                created_at: s.created_at.to_string(),
                last_activity_at: s.last_activity_at.to_string(),
            }).unwrap()),
        ),
        Ok(None) => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "not found"}))),
        Err(e) => {
            warn!(error = %e, "hands: session query error");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": "internal"})))
        }
    }
}

/// DELETE /v1/hands/sessions/:id
pub async fn close_session(
    State(state): State<Arc<HandsState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let _ = hands_db::close_session(&state.pool, &session_id, "user_closed").await;
    state.session_manager.close(&session_id);
    StatusCode::OK
}

/// POST /v1/hands/sessions/:id/activated
pub async fn activate_session(
    State(state): State<Arc<HandsState>>,
    Path(session_id): Path<String>,
    Json(req): Json<ActivateSessionRequest>,
) -> impl IntoResponse {
    if let Err(e) = hands_db::activate_session(
        &state.pool,
        &session_id,
        &req.browser_session_id,
        req.device_id.as_deref(),
        req.host_os.as_deref(),
    )
    .await
    {
        warn!(error = %e, "hands: failed to activate session");
        return StatusCode::INTERNAL_SERVER_ERROR;
    }

    state.session_manager.activate(&session_id);

    info!(session_id = session_id, "hands: session activated (OnlyKey confirmed)");
    StatusCode::OK
}

/// POST /v1/hands/sessions/:id/emergency-stop
pub async fn emergency_stop(
    State(state): State<Arc<HandsState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let _ = hands_db::close_session(&state.pool, &session_id, "emergency_stop").await;
    state.session_manager.close(&session_id);

    // Find the session's job and mark it paused
    if let Ok(Some(session)) = hands_db::find_session(&state.pool, &session_id).await {
        let _ = hands_db::update_job_status(&state.pool, &session.job_id, "paused").await;
        let _ = hands_db::insert_audit_event(
            &state.pool, &generate_id("ha"), &session.job_id,
            "emergency_stop", None, Some(&session_id), None,
        ).await;
    }

    warn!(session_id = session_id, "hands: EMERGENCY STOP");
    StatusCode::OK
}

// ── Instruction delivery endpoints ─────────────────────────────────────

/// GET /v1/hands/sessions/:id/next-packet
///
/// Browser polls this to get the next instruction packet to deliver via WebHID.
pub async fn next_packet(
    State(state): State<Arc<HandsState>>,
    Path(session_id): Path<String>,
) -> impl IntoResponse {
    let _ = hands_db::touch_session(&state.pool, &session_id).await;

    // Check for queued packets first
    if let Some(packet) = state.session_manager.dequeue_packet(&session_id) {
        return (
            StatusCode::OK,
            Json(serde_json::to_value(NextPacketResponse {
                packet_id: packet.packet_id,
                step_id: packet.step_id,
                cbor_b64: packet.cbor_b64,
                flags: packet.flags,
            })
            .unwrap()),
        );
    }

    // No packets queued — check if there's a pending step to compile
    let session = match state.session_manager.sessions.get(&session_id) {
        Some(s) => s,
        None => return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "session not found"}))),
    };

    if session.status != SessionStatus::Active {
        return (StatusCode::NO_CONTENT, Json(serde_json::json!(null)));
    }

    let job_id = session.job_id.clone();
    drop(session); // release DashMap ref

    // Find the next pending step
    let step = match hands_db::find_next_pending_step(&state.pool, &job_id).await {
        Ok(Some(s)) => s,
        _ => return (StatusCode::NO_CONTENT, Json(serde_json::json!(null))),
    };

    // Determine host OS
    let host_os = hands_db::find_job(&state.pool, &job_id)
        .await
        .ok()
        .flatten()
        .and_then(|j| j.host_os)
        .and_then(|os| HostOS::from_str(&os))
        .unwrap_or(HostOS::Linux);

    // Parse macro instructions
    let macros: Vec<MacroInstruction> =
        serde_json::from_value(step.macro_instructions.clone()).unwrap_or_default();

    // Compile to raw keystrokes
    let keystrokes = compile::compile_instruction_set(&macros, &host_os);

    // Encode as CBOR
    let instruction_packet = packet::InstructionPacket {
        session_id: session_id.clone(),
        step_id: step.id.clone(),
        instructions: keystrokes,
        expect_screenshot: true,
        timeout_ms: step.timeout_ms as u32,
    };

    let cbor = packet::encode_cbor(&instruction_packet).unwrap_or_default();
    let cbor_b64 = base64::engine::general_purpose::STANDARD.encode(&cbor);

    // Mark step as sending
    let _ = hands_db::update_step_status(&state.pool, &step.id, "sending", None).await;

    let flags = if step.require_confirm {
        packet::FLAG_ENCRYPTED | packet::FLAG_REQUIRES_CONFIRM
    } else {
        packet::FLAG_ENCRYPTED
    };

    let packet_id = generate_id("hp");

    (
        StatusCode::OK,
        Json(serde_json::to_value(NextPacketResponse {
            packet_id,
            step_id: step.id,
            cbor_b64,
            flags,
        })
        .unwrap()),
    )
}

/// POST /v1/hands/sessions/:id/packet-acked
pub async fn packet_acked(
    State(state): State<Arc<HandsState>>,
    Path(session_id): Path<String>,
    Json(req): Json<PacketAckRequest>,
) -> impl IntoResponse {
    let _ = hands_db::touch_session(&state.pool, &session_id).await;
    info!(session_id = session_id, packet_id = req.packet_id, "hands: packet acknowledged");
    StatusCode::OK
}

/// POST /v1/hands/sessions/:id/step-status
pub async fn step_status(
    State(state): State<Arc<HandsState>>,
    Path(session_id): Path<String>,
    Json(req): Json<StepStatusReport>,
) -> impl IntoResponse {
    let _ = hands_db::touch_session(&state.pool, &session_id).await;

    let status = match req.status_code {
        0x00 => "pending",  // queued on device
        0x01 => "executing",
        0x02 => "succeeded",
        0x03 => "failed",
        0x04 => {
            // Button stop → emergency stop the whole session
            warn!(session_id = session_id, "hands: button stop detected");
            let _ = hands_db::close_session(&state.pool, &session_id, "button_stop").await;
            state.session_manager.close(&session_id);
            "failed"
        }
        _ => "failed",
    };

    let _ = hands_db::update_step_status(
        &state.pool,
        &req.step_id,
        status,
        req.detail.as_deref(),
    )
    .await;

    info!(
        session_id = session_id,
        step_id = req.step_id,
        status_code = req.status_code,
        "hands: step status update"
    );

    StatusCode::OK
}

// ── Screenshot endpoints ───────────────────────────────────────────────

/// POST /v1/hands/screenshots
pub async fn upload_screenshot(
    State(state): State<Arc<HandsState>>,
    axum::extract::Multipart(mut multipart): axum::extract::Multipart,
) -> impl IntoResponse {
    let mut session_id = String::new();
    let mut step_id = String::new();
    let mut image_data: Vec<u8> = Vec::new();
    let mut width = 0i32;
    let mut height = 0i32;

    while let Ok(Some(field)) = multipart.next_field().await {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "session_id" => session_id = field.text().await.unwrap_or_default(),
            "step_id" => step_id = field.text().await.unwrap_or_default(),
            "width" => width = field.text().await.unwrap_or_default().parse().unwrap_or(0),
            "height" => height = field.text().await.unwrap_or_default().parse().unwrap_or(0),
            "image" => image_data = field.bytes().await.unwrap_or_default().to_vec(),
            _ => {}
        }
    }

    if session_id.is_empty() || step_id.is_empty() || image_data.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "missing required fields"})),
        );
    }

    let screenshot_id = generate_id("hsc");
    if let Err(e) = hands_db::insert_screenshot(
        &state.pool,
        &screenshot_id,
        &session_id,
        &step_id,
        &image_data,
        width,
        height,
    )
    .await
    {
        warn!(error = %e, "hands: failed to store screenshot");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "failed to store"})),
        );
    }

    info!(
        screenshot_id = screenshot_id,
        session_id = session_id,
        size_bytes = image_data.len(),
        "hands: screenshot uploaded"
    );

    (
        StatusCode::CREATED,
        Json(serde_json::json!({ "id": screenshot_id })),
    )
}

/// GET /v1/hands/screenshots/:id
pub async fn get_screenshot(
    State(_state): State<Arc<HandsState>>,
    Path(screenshot_id): Path<String>,
) -> impl IntoResponse {
    // TODO: query screenshot by ID and return image bytes
    // For now, return 501
    (StatusCode::NOT_IMPLEMENTED, Json(serde_json::json!({"error": "not yet implemented"})))
}

// ── Helpers ────────────────────────────────────────────────────────────

fn generate_id(prefix: &str) -> String {
    format!("{}_{}", prefix, ulid::Ulid::new().to_string().to_lowercase())
}
