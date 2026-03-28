/**
 * Shared types for the OnlyAgent Hands system.
 *
 * These types mirror the Rust gateway models and Prisma schema
 * for the remote machine control layer using OnlyKey via WebHID.
 */

// ── Enums ───────────────────────────────────────────────────────────────

export type JobStatus =
  | "draft"
  | "queued"
  | "running"
  | "paused"
  | "completed"
  | "failed"
  | "cancelled";

export type StepStatus =
  | "pending"
  | "sending"
  | "executing"
  | "verifying"
  | "succeeded"
  | "failed"
  | "skipped";

export type SessionStatus = "establishing" | "active" | "paused" | "closed";

export type HostOS = "macos" | "windows" | "linux";

// ── Macro instructions (AI agent generates these) ──────────────────────

export type MacroInstruction =
  | { macro: "open_browser"; browser?: string }
  | { macro: "navigate_url"; url: string }
  | { macro: "open_terminal" }
  | { macro: "run_command"; command: string }
  | { macro: "screenshot" }
  | { macro: "switch_window" }
  | { macro: "close_window" }
  | { macro: "select_all" }
  | { macro: "copy" }
  | { macro: "paste" }
  | { macro: "save" }
  | { macro: "undo" }
  | { macro: "find"; text: string }
  | { macro: "type_text"; text: string }
  | { macro: "wait"; seconds: number }
  | { macro: "key_press"; key: string; modifiers?: string[] }
  | { macro: "key_combo"; keys: string[]; modifiers?: string[] };

// ── Job ────────────────────────────────────────────────────────────────

export interface JobSummary {
  id: string;
  name: string;
  description: string;
  status: JobStatus;
  host_os: HostOS | null;
  step_count: number;
  created_at: string;
}

export interface JobDetail {
  id: string;
  name: string;
  description: string;
  status: JobStatus;
  host_os: HostOS | null;
  max_duration_secs: number;
  steps: StepView[];
  created_at: string;
  updated_at: string;
  completed_at: string | null;
}

export interface StepView {
  id: string;
  sequence_number: number;
  description: string;
  macro_instructions: MacroInstruction[];
  expected_outcome: string;
  status: StepStatus;
  retry_count: number;
  max_retries: number;
  timeout_ms: number;
  require_confirm: boolean;
  error_message: string | null;
}

// ── Session ────────────────────────────────────────────────────────────

export interface CreateSessionResponse {
  session_id: string;
  nonce: string;
  agent_token: string;
}

export interface SessionView {
  id: string;
  job_id: string;
  status: SessionStatus;
  host_os: HostOS | null;
  created_at: string;
  last_activity_at: string;
}

// ── Instruction delivery ───────────────────────────────────────────────

export interface NextPacketResponse {
  packet_id: string;
  step_id: string;
  cbor_b64: string;
  flags: number;
}

export interface StepStatusReport {
  step_id: string;
  status_code: number;
  detail?: string;
}

// ── Screenshot ─────────────────────────────────────────────────────────

export interface ScreenshotMeta {
  id: string;
  session_id: string;
  step_id: string;
  captured_at: string;
  width: number;
  height: number;
  size_bytes: number;
  analysis: unknown;
}

// ── API input types ────────────────────────────────────────────────────

export interface CreateJobInput {
  name: string;
  description: string;
  host_os?: HostOS;
  steps: CreateStepInput[];
}

export interface CreateStepInput {
  description: string;
  macro_instructions: MacroInstruction[];
  expected_outcome: string;
  max_retries?: number;
  timeout_ms?: number;
  require_confirm?: boolean;
}
