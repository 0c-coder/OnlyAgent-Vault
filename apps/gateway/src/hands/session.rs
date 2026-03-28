//! Session lifecycle management for Hands control sessions.

use std::collections::VecDeque;
use std::time::Instant;

use dashmap::DashMap;

use super::models::SessionStatus;

/// An active session tracked in memory (packet queue, timing).
pub struct ActiveSession {
    pub session_id: String,
    pub job_id: String,
    pub current_step_id: Option<String>,
    /// Compiled CBOR packets awaiting browser pickup.
    pub packet_queue: VecDeque<QueuedPacket>,
    pub status: SessionStatus,
    pub created_at: Instant,
    pub last_activity: Instant,
}

/// A queued instruction packet ready for WebHID delivery.
pub struct QueuedPacket {
    pub packet_id: String,
    pub step_id: String,
    /// Base64-encoded CBOR payload
    pub cbor_b64: String,
    pub flags: u8,
    pub created_at: Instant,
}

/// In-memory session manager.
pub struct SessionManager {
    pub sessions: DashMap<String, ActiveSession>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }

    /// Register a new active session.
    pub fn register(&self, session_id: String, job_id: String) {
        let now = Instant::now();
        self.sessions.insert(
            session_id.clone(),
            ActiveSession {
                session_id,
                job_id,
                current_step_id: None,
                packet_queue: VecDeque::new(),
                status: SessionStatus::Establishing,
                created_at: now,
                last_activity: now,
            },
        );
    }

    /// Mark session as active (OnlyKey button confirmed).
    pub fn activate(&self, session_id: &str) -> bool {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.status = SessionStatus::Active;
            session.last_activity = Instant::now();
            true
        } else {
            false
        }
    }

    /// Enqueue a compiled instruction packet for browser pickup.
    pub fn enqueue_packet(&self, session_id: &str, packet: QueuedPacket) -> bool {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.current_step_id = Some(packet.step_id.clone());
            session.packet_queue.push_back(packet);
            session.last_activity = Instant::now();
            true
        } else {
            false
        }
    }

    /// Dequeue the next packet for browser delivery.
    pub fn dequeue_packet(&self, session_id: &str) -> Option<QueuedPacket> {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.last_activity = Instant::now();
            session.packet_queue.pop_front()
        } else {
            None
        }
    }

    /// Close a session.
    pub fn close(&self, session_id: &str) {
        if let Some(mut session) = self.sessions.get_mut(session_id) {
            session.status = SessionStatus::Closed;
        }
    }

    /// Check if a session is active.
    pub fn is_active(&self, session_id: &str) -> bool {
        self.sessions
            .get(session_id)
            .map(|s| s.status == SessionStatus::Active)
            .unwrap_or(false)
    }

    /// Cleanup stale sessions (idle > timeout).
    pub fn cleanup_stale(&self, timeout_secs: u64) -> usize {
        let cutoff = Instant::now() - std::time::Duration::from_secs(timeout_secs);
        let stale: Vec<String> = self
            .sessions
            .iter()
            .filter(|entry| {
                entry.last_activity < cutoff
                    && entry.status != SessionStatus::Closed
            })
            .map(|entry| entry.session_id.clone())
            .collect();

        let count = stale.len();
        for id in stale {
            if let Some(mut session) = self.sessions.get_mut(&id) {
                session.status = SessionStatus::Closed;
            }
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_activate() {
        let mgr = SessionManager::new();
        mgr.register("s1".into(), "j1".into());
        assert!(!mgr.is_active("s1"));
        mgr.activate("s1");
        assert!(mgr.is_active("s1"));
    }

    #[test]
    fn enqueue_dequeue() {
        let mgr = SessionManager::new();
        mgr.register("s1".into(), "j1".into());
        mgr.activate("s1");

        let packet = QueuedPacket {
            packet_id: "p1".into(),
            step_id: "step1".into(),
            cbor_b64: "AAAA".into(),
            flags: 0x01,
            created_at: Instant::now(),
        };
        mgr.enqueue_packet("s1", packet);

        let dequeued = mgr.dequeue_packet("s1");
        assert!(dequeued.is_some());
        assert_eq!(dequeued.unwrap().packet_id, "p1");

        // Queue should be empty now
        assert!(mgr.dequeue_packet("s1").is_none());
    }

    #[test]
    fn close_session() {
        let mgr = SessionManager::new();
        mgr.register("s1".into(), "j1".into());
        mgr.activate("s1");
        assert!(mgr.is_active("s1"));
        mgr.close("s1");
        assert!(!mgr.is_active("s1"));
    }
}
