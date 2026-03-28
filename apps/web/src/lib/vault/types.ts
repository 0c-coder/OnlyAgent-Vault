/**
 * Shared types for the OnlyKey Vault system.
 *
 * These types mirror the Rust models and Prisma schema for the vault
 * record protection layer using OnlyKey hardware-derived encryption.
 */

// ── Enums ───────────────────────────────────────────────────────────────

export type RecordType =
  | "api_key"
  | "oauth_token"
  | "age_secret"
  | "generic_secret";

export type CacheScope = "global" | "agent" | "session";

export type ApprovalStatus = "pending" | "approved" | "denied" | "expired";

export type BrowserOperation =
  | "unlock_record_key"
  | "decrypt_age"
  | "sign_blob";

export type RevocationReason =
  | "manual_revoke"
  | "ttl_expired"
  | "idle_timeout"
  | "browser_disconnect"
  | "policy_changed"
  | "key_rotated"
  | "server_restart"
  | "admin_revoke";

// ── Derivation context ──────────────────────────────────────────────────

/** Context passed to ok.derive_shared_secret as AdditionalData. */
export interface DerivationContext {
  record_id: string;
  purpose: string;
  version: number;
  tenant_id?: string;
  origin?: string;
}

// ── Record policy ───────────────────────────────────────────────────────

export interface RecordPolicy {
  requireOnlykey: boolean;
  unlockTtlSeconds: number;
  idleTimeoutSeconds: number;
  cacheScope: CacheScope;
  allowManualRevoke: boolean;
  relockOnBrowserDisconnect: boolean;
  relockOnPolicyChange: boolean;
  requireFreshUnlockForHighRisk: boolean;
  allowPlaintextReturn: boolean;
  allowedAgents: string[];
}

// ── Vault record (as seen by the web UI) ────────────────────────────────

export interface VaultRecord {
  id: string;
  name: string;
  recordType: RecordType;
  hostPattern: string;
  pathPattern: string | null;
  policy: RecordPolicy;
  recordVersion: number;
  policyVersion: number;
  keyVersion: number;
  unlockGeneration: number;
  createdAt: string;
  updatedAt: string;
}

// ── Pending approval request ────────────────────────────────────────────

/** Pending approval as returned by the gateway's browser endpoint. */
export interface PendingApprovalRequest {
  request_id: string;
  record_id: string;
  record_name: string;
  agent_id: string;
  operation: BrowserOperation;
  origin: string;
  onecli_record_pubkey_jwk: JsonWebKey;
  additional_data: DerivationContext;
  created_at: string;
  expires_at: string;
  nonce_b64: string;
}

// ── Browser approve payload ─────────────────────────────────────────────

export interface BrowserApprovePayload {
  request_id: string;
  derived_secret_b64: string;
  browser_session_id: string;
}

// ── API responses ───────────────────────────────────────────────────────

export interface AccessRecordResponse {
  status: "ok" | "pending_approval" | "denied";
  request_id?: string;
  secret?: string;
  expires_at?: string;
}

export interface BrowserApproveResponse {
  status: "ok" | "error";
  message?: string;
}

export interface PendingRequestsResponse {
  items: PendingApprovalRequest[];
}

// ── Vault record creation input ─────────────────────────────────────────

export interface CreateVaultRecordInput {
  name: string;
  recordType: RecordType;
  secretValue: string;
  hostPattern: string;
  pathPattern?: string;
  injectionConfig?: {
    headerName: string;
    valueFormat?: string;
  };
  policy?: Partial<RecordPolicy>;
}
