/**
 * OnlyKey integration for the Vault browser bridge.
 *
 * Uses the real `node-onlykey` library to communicate with OnlyKey
 * via the FIDO2/WebAuthn protocol. This runs in the browser at a
 * trusted origin (apps.crp.to or custom enrolled origin).
 *
 * The key operation is `ok.derive_shared_secret()` which deterministically
 * derives a shared secret from:
 *   - The web origin (RPID)
 *   - The input public key (OneCLI per-record pubkey)
 *   - AdditionalData (derivation context: record_id, purpose, version)
 *
 * Given the same inputs and origin, the same secret is reproduced.
 * This is what allows OneCLI to store NO private keys.
 *
 * @see https://github.com/trustcrypto/node-onlykey
 */

import type { DerivationContext } from "./types";

// ── OnlyKey instance type ───────────────────────────────────────────────

/**
 * Minimal interface for the node-onlykey library's OnlyKey class.
 * The actual library exports more methods, but we only need these.
 */
export interface OnlyKeyInstance {
  derive_shared_secret: (
    additionalData: string,
    inputPubkeyJwk: JsonWebKey,
    keyType: string,
    pressRequired: boolean,
  ) => Promise<string>; // returns base64-encoded shared secret

  // Connection lifecycle
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  isConnected: () => boolean;
}

// ── Derive shared secret for a vault record ─────────────────────────────

export interface DeriveSecretArgs {
  /** The OnlyKey instance (from node-onlykey library) */
  ok: OnlyKeyInstance;
  /** Per-record OneCLI public key in JWK format */
  onecliRecordPubkeyJwk: JsonWebKey;
  /** Derivation context — serialized as AdditionalData */
  additionalData: DerivationContext;
  /** Key type for the derivation (default: "P-256") */
  keyType?: string;
  /** Whether physical press on OnlyKey is required (default: true) */
  pressRequired?: boolean;
}

export interface DeriveSecretResult {
  /** Base64-encoded derived shared secret */
  derivedSecretB64: string;
}

/**
 * Derive a shared secret from OnlyKey for a specific vault record.
 *
 * This calls `ok.derive_shared_secret()` with the record's public key
 * and derivation context. The result is deterministic for the same
 * inputs and origin.
 *
 * @throws If OnlyKey is not connected or derivation fails
 */
export const deriveSecretForRecord = async (
  args: DeriveSecretArgs,
): Promise<DeriveSecretResult> => {
  const {
    ok,
    onecliRecordPubkeyJwk,
    additionalData,
    keyType = "P-256",
    pressRequired = true,
  } = args;

  if (!ok.isConnected()) {
    throw new Error(
      "OnlyKey is not connected. Please connect your OnlyKey and try again.",
    );
  }

  // Serialize the derivation context as the AdditionalData parameter.
  // This binds the derived secret to the specific record, purpose, and version.
  const additionalDataStr = JSON.stringify(additionalData);

  const derivedSecretB64 = await ok.derive_shared_secret(
    additionalDataStr,
    onecliRecordPubkeyJwk,
    keyType,
    pressRequired,
  );

  return { derivedSecretB64 };
};

// ── Derive shared secret for record creation ────────────────────────────

/**
 * During record creation, the browser derives the shared secret so the
 * server can wrap the random record key. This is the same derivation
 * but used in the "setup" direction rather than "unlock" direction.
 */
export const deriveSecretForRecordCreation = async (
  ok: OnlyKeyInstance,
  onecliPubkeyJwk: JsonWebKey,
  recordId: string,
  version: number = 1,
): Promise<DeriveSecretResult> => {
  const additionalData: DerivationContext = {
    record_id: recordId,
    purpose: "record_key_wrap",
    version,
  };

  return deriveSecretForRecord({
    ok,
    onecliRecordPubkeyJwk: onecliPubkeyJwk,
    additionalData,
    pressRequired: true,
  });
};

// ── OnlyKey connection helper ───────────────────────────────────────────

/**
 * Initialize and connect to OnlyKey via the FIDO2 bridge.
 *
 * In production, this imports the real `node-onlykey` library.
 * The library uses WebAuthn/FIDO2 to tunnel data to the device.
 */
export const connectOnlyKey = async (): Promise<OnlyKeyInstance> => {
  // Dynamic import of node-onlykey (loaded from the trusted origin)
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  const { OnlyKey } = await import("node-onlykey");
  const ok = new OnlyKey() as OnlyKeyInstance;
  await ok.connect();
  return ok;
};
