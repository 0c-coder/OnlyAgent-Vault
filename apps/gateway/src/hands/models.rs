//! Data models for the OnlyAgent Hands system.

use serde::{Deserialize, Serialize};

// ── Job status ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Draft,
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "draft" => Some(Self::Draft),
            "queued" => Some(Self::Queued),
            "running" => Some(Self::Running),
            "paused" => Some(Self::Paused),
            "completed" => Some(Self::Completed),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

// ── Step status ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Pending,
    Sending,
    Executing,
    Verifying,
    Succeeded,
    Failed,
    Skipped,
}

impl StepStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Sending => "sending",
            Self::Executing => "executing",
            Self::Verifying => "verifying",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

// ── Session status ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Establishing,
    Active,
    Paused,
    Closed,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Establishing => "establishing",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Closed => "closed",
        }
    }
}

// ── Host OS ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum HostOS {
    MacOS,
    Windows,
    Linux,
}

impl HostOS {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MacOS => "macos",
            Self::Windows => "windows",
            Self::Linux => "linux",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "macos" => Some(Self::MacOS),
            "windows" => Some(Self::Windows),
            "linux" => Some(Self::Linux),
            _ => None,
        }
    }
}

// ── Macro instructions (AI agent generates these) ──────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "macro", rename_all = "snake_case")]
pub enum MacroInstruction {
    OpenBrowser {
        #[serde(skip_serializing_if = "Option::is_none")]
        browser: Option<String>,
    },
    NavigateUrl {
        url: String,
    },
    OpenTerminal,
    RunCommand {
        command: String,
    },
    Screenshot,
    SwitchWindow,
    CloseWindow,
    SelectAll,
    Copy,
    Paste,
    Save,
    Undo,
    Find {
        text: String,
    },
    TypeText {
        text: String,
    },
    Wait {
        seconds: u32,
    },
    KeyPress {
        key: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        modifiers: Option<Vec<String>>,
    },
    KeyCombo {
        keys: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        modifiers: Option<Vec<String>>,
    },
}

// ── Raw HID keystroke (compiled output) ────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum RawKeystroke {
    /// Type a string of text character by character
    TypeText {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        delay_per_char_ms: Option<u32>,
    },
    /// Press a single key
    Key {
        code: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        mods: Option<u8>,
    },
    /// Press a key combination
    Combo {
        codes: Vec<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        mods: Option<u8>,
    },
    /// Delay in milliseconds
    Delay {
        ms: u32,
    },
}

// ── HID modifier bitmask ───────────────────────────────────────────────

pub const MOD_CTRL: u8 = 0x01;
pub const MOD_SHIFT: u8 = 0x02;
pub const MOD_ALT: u8 = 0x04;
pub const MOD_GUI: u8 = 0x08; // Cmd on macOS, Win on Windows, Super on Linux

// ── HID key codes (USB HID Usage Table: Keyboard/Keypad Page 0x07) ────

pub const KEY_A: u8 = 0x04;
pub const KEY_L: u8 = 0x0F;
pub const KEY_T: u8 = 0x17;
pub const KEY_ENTER: u8 = 0x28;
pub const KEY_ESCAPE: u8 = 0x29;
pub const KEY_SPACE: u8 = 0x2C;
pub const KEY_TAB: u8 = 0x2B;
pub const KEY_BACKSPACE: u8 = 0x2A;
pub const KEY_F1: u8 = 0x3A;
pub const KEY_F3: u8 = 0x3C;
pub const KEY_F5: u8 = 0x3E;
pub const KEY_PRINTSCREEN: u8 = 0x46;
pub const KEY_DELETE: u8 = 0x4C;
pub const KEY_RIGHT: u8 = 0x4F;
pub const KEY_LEFT: u8 = 0x50;
pub const KEY_DOWN: u8 = 0x51;
pub const KEY_UP: u8 = 0x52;

// ── API request/response types ─────────────────────────────────────────

/// Create a new job.
#[derive(Debug, Deserialize)]
pub struct CreateJobRequest {
    pub name: String,
    pub description: String,
    pub host_os: Option<String>,
    pub steps: Vec<CreateStepInput>,
}

#[derive(Debug, Deserialize)]
pub struct CreateStepInput {
    pub description: String,
    pub macro_instructions: serde_json::Value, // MacroInstruction[]
    pub expected_outcome: String,
    #[serde(default = "default_max_retries")]
    pub max_retries: i32,
    #[serde(default = "default_timeout_ms")]
    pub timeout_ms: i32,
    #[serde(default)]
    pub require_confirm: bool,
}

fn default_max_retries() -> i32 {
    3
}
fn default_timeout_ms() -> i32 {
    30000
}

/// Create a new session.
#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub job_id: String,
    #[serde(default)]
    pub host_os: Option<String>,
}

/// Session creation response.
#[derive(Debug, Serialize)]
pub struct CreateSessionResponse {
    pub session_id: String,
    pub nonce: String, // base64-encoded nonce for WebHID session auth
    pub agent_token: String,
}

/// Session activation (browser reports OnlyKey button press).
#[derive(Debug, Deserialize)]
pub struct ActivateSessionRequest {
    pub browser_session_id: String,
    #[serde(default)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub host_os: Option<String>,
}

/// Next instruction packet for browser delivery via WebHID.
#[derive(Debug, Serialize)]
pub struct NextPacketResponse {
    pub packet_id: String,
    pub step_id: String,
    /// Base64-encoded CBOR instruction payload
    pub cbor_b64: String,
    /// HID report flags (0x01=encrypted, 0x02=requires_confirm)
    pub flags: u8,
}

/// Browser reports device acknowledged receipt.
#[derive(Debug, Deserialize)]
pub struct PacketAckRequest {
    pub packet_id: String,
}

/// Browser forwards device status report.
#[derive(Debug, Deserialize)]
pub struct StepStatusReport {
    pub step_id: String,
    /// 0x00=queued, 0x01=executing, 0x02=complete, 0x03=error, 0x04=button_stop
    pub status_code: u8,
    #[serde(default)]
    pub detail: Option<String>,
}

/// Job summary (list view).
#[derive(Debug, Serialize)]
pub struct JobSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub host_os: Option<String>,
    pub step_count: i64,
    pub created_at: String,
}

/// Job detail with steps.
#[derive(Debug, Serialize)]
pub struct JobDetail {
    pub id: String,
    pub name: String,
    pub description: String,
    pub status: String,
    pub host_os: Option<String>,
    pub max_duration_secs: i32,
    pub steps: Vec<StepView>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct StepView {
    pub id: String,
    pub sequence_number: i32,
    pub description: String,
    pub macro_instructions: serde_json::Value,
    pub expected_outcome: String,
    pub status: String,
    pub retry_count: i32,
    pub max_retries: i32,
    pub timeout_ms: i32,
    pub require_confirm: bool,
    pub error_message: Option<String>,
}

/// Session view.
#[derive(Debug, Serialize)]
pub struct SessionView {
    pub id: String,
    pub job_id: String,
    pub status: String,
    pub host_os: Option<String>,
    pub created_at: String,
    pub last_activity_at: String,
}

/// Screenshot metadata.
#[derive(Debug, Serialize)]
pub struct ScreenshotMeta {
    pub id: String,
    pub session_id: String,
    pub step_id: String,
    pub captured_at: String,
    pub width: i32,
    pub height: i32,
    pub size_bytes: i32,
    pub analysis: Option<serde_json::Value>,
}

// ── Audit events ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct AuditEvent {
    pub event: String,
    pub job_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub timestamp: String,
}
