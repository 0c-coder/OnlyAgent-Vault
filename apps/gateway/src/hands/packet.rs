//! HID report packet framing.
//!
//! Serializes compiled keystroke instructions into CBOR and frames them
//! into 59-byte chunks suitable for WebHID report delivery.

use serde::Serialize;

use super::models::RawKeystroke;

/// Maximum payload per HID report (64 bytes total - 5 byte header).
pub const HID_PAYLOAD_SIZE: usize = 59;

/// HID report IDs for the Hands protocol.
pub const REPORT_ID_INSTRUCTION: u8 = 0x70;
pub const REPORT_ID_SESSION_AUTH: u8 = 0x71;
pub const REPORT_ID_ACK: u8 = 0x72;
pub const REPORT_ID_STATUS: u8 = 0x73;
pub const REPORT_ID_EMERGENCY_STOP: u8 = 0x74;
pub const REPORT_ID_PING: u8 = 0x75;

/// HID report flags.
pub const FLAG_ENCRYPTED: u8 = 0x01;
pub const FLAG_REQUIRES_CONFIRM: u8 = 0x02;
pub const FLAG_HIGH_RISK: u8 = 0x04;

/// A complete instruction packet to be serialized to CBOR and chunked.
#[derive(Debug, Serialize)]
pub struct InstructionPacket {
    pub session_id: String,
    pub step_id: String,
    pub instructions: Vec<RawKeystroke>,
    pub expect_screenshot: bool,
    pub timeout_ms: u32,
}

/// A single HID report frame (one chunk of a larger instruction packet).
#[derive(Debug)]
pub struct HidReportFrame {
    pub seq_no: u16,
    pub total: u16,
    pub flags: u8,
    /// Up to 59 bytes of CBOR payload
    pub payload: Vec<u8>,
}

impl HidReportFrame {
    /// Serialize this frame into a 64-byte HID report body (without report ID).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = vec![0u8; 64];
        buf[0] = (self.seq_no >> 8) as u8;
        buf[1] = (self.seq_no & 0xFF) as u8;
        buf[2] = (self.total >> 8) as u8;
        buf[3] = (self.total & 0xFF) as u8;
        buf[4] = self.flags;
        let copy_len = self.payload.len().min(HID_PAYLOAD_SIZE);
        buf[5..5 + copy_len].copy_from_slice(&self.payload[..copy_len]);
        buf
    }
}

/// Encode an instruction packet as CBOR bytes.
pub fn encode_cbor(packet: &InstructionPacket) -> Result<Vec<u8>, serde_json::Error> {
    // We use JSON as a CBOR stand-in since serde_cbor may not be available yet.
    // In production, swap to serde_cbor::to_vec().
    serde_json::to_vec(packet)
}

/// Split a CBOR byte array into HID report frames.
pub fn frame_cbor(cbor: &[u8], flags: u8) -> Vec<HidReportFrame> {
    if cbor.is_empty() {
        return vec![];
    }

    let total_frames = (cbor.len() + HID_PAYLOAD_SIZE - 1) / HID_PAYLOAD_SIZE;

    (0..total_frames)
        .map(|i| {
            let start = i * HID_PAYLOAD_SIZE;
            let end = (start + HID_PAYLOAD_SIZE).min(cbor.len());
            HidReportFrame {
                seq_no: i as u16,
                total: total_frames as u16,
                flags,
                payload: cbor[start..end].to_vec(),
            }
        })
        .collect()
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_single_packet() {
        let data = vec![0xAA; 40]; // fits in one frame
        let frames = frame_cbor(&data, FLAG_ENCRYPTED);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].seq_no, 0);
        assert_eq!(frames[0].total, 1);
        assert_eq!(frames[0].flags, FLAG_ENCRYPTED);
        assert_eq!(frames[0].payload.len(), 40);
    }

    #[test]
    fn frame_multi_packet() {
        let data = vec![0xBB; 150]; // needs 3 frames (59+59+32)
        let frames = frame_cbor(&data, 0);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].seq_no, 0);
        assert_eq!(frames[0].total, 3);
        assert_eq!(frames[0].payload.len(), 59);
        assert_eq!(frames[1].payload.len(), 59);
        assert_eq!(frames[2].payload.len(), 32);
    }

    #[test]
    fn frame_empty() {
        let frames = frame_cbor(&[], 0);
        assert!(frames.is_empty());
    }

    #[test]
    fn hid_report_frame_to_bytes() {
        let frame = HidReportFrame {
            seq_no: 1,
            total: 3,
            flags: FLAG_ENCRYPTED | FLAG_REQUIRES_CONFIRM,
            payload: vec![0xDE, 0xAD],
        };
        let bytes = frame.to_bytes();
        assert_eq!(bytes.len(), 64);
        assert_eq!(bytes[0], 0); // seq_no high
        assert_eq!(bytes[1], 1); // seq_no low
        assert_eq!(bytes[2], 0); // total high
        assert_eq!(bytes[3], 3); // total low
        assert_eq!(bytes[4], 0x03); // flags
        assert_eq!(bytes[5], 0xDE);
        assert_eq!(bytes[6], 0xAD);
        assert_eq!(bytes[7], 0x00); // zero-padded
    }
}
