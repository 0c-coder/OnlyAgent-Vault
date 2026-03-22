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
