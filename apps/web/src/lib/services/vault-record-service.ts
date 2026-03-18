import { db, Prisma } from "@onecli/db";
import { ServiceError } from "@/lib/services/errors";
import type { CreateVaultRecordInput } from "@/lib/vault/types";

/**
 * List all vault records for a user (metadata only, no ciphertext).
 */
export const listVaultRecords = async (userId: string) => {
  const records = await db.vaultRecord.findMany({
    where: { userId },
    select: {
      id: true,
      name: true,
      recordType: true,
      hostPattern: true,
      pathPattern: true,
      requireOnlykey: true,
      unlockTtlSeconds: true,
      idleTimeoutSeconds: true,
      cacheScope: true,
      recordVersion: true,
      policyVersion: true,
      keyVersion: true,
      createdAt: true,
      updatedAt: true,
    },
    orderBy: { createdAt: "desc" },
  });

  return records;
};

/**
 * Create a new vault record.
 *
 * NOTE: The actual encryption happens on the browser+gateway side.
 * The browser derives the shared secret via OnlyKey, then the gateway
 * generates a random record key, encrypts the secret, wraps the record key,
 * and stores everything. This server action just creates the DB row
 * with the pre-encrypted data from the gateway.
 */
export interface CreateVaultRecordData {
  name: string;
  recordType: string;
  hostPattern: string;
  pathPattern?: string | null;
  injectionConfig?: { headerName: string; valueFormat?: string } | null;

  // Pre-encrypted by the gateway after OnlyKey derivation
  ciphertextB64: string;
  nonceB64: string;
  aadJson: string;
  wrappedKeyB64: string;
  wrappedKeyNonceB64: string;
  onecliPubkeyJwk: string;
  derivationContext: string;

  // Optional policy overrides
  unlockTtlSeconds?: number;
  idleTimeoutSeconds?: number;
  cacheScope?: string;
}

export const createVaultRecord = async (
  userId: string,
  data: CreateVaultRecordData,
) => {
  const name = data.name.trim();
  if (!name || name.length > 255) {
    throw new ServiceError(
      "BAD_REQUEST",
      "Name must be between 1 and 255 characters",
    );
  }

  if (!data.hostPattern.trim()) {
    throw new ServiceError("BAD_REQUEST", "Host pattern is required");
  }

  const record = await db.vaultRecord.create({
    data: {
      name,
      recordType: data.recordType,
      ciphertextB64: data.ciphertextB64,
      nonceB64: data.nonceB64,
      aadJson: data.aadJson,
      wrappedKeyB64: data.wrappedKeyB64,
      wrappedKeyNonceB64: data.wrappedKeyNonceB64,
      onecliPubkeyJwk: data.onecliPubkeyJwk,
      derivationContext: data.derivationContext,
      hostPattern: data.hostPattern.trim(),
      pathPattern: data.pathPattern?.trim() || null,
      injectionConfig: data.injectionConfig
        ? (data.injectionConfig as unknown as Prisma.InputJsonValue)
        : Prisma.JsonNull,
      unlockTtlSeconds: data.unlockTtlSeconds ?? 86400,
      idleTimeoutSeconds: data.idleTimeoutSeconds ?? 3600,
      cacheScope: data.cacheScope ?? "agent",
      userId,
    },
    select: {
      id: true,
      name: true,
      recordType: true,
      hostPattern: true,
      createdAt: true,
    },
  });

  return record;
};

/**
 * Delete a vault record.
 */
export const deleteVaultRecord = async (
  userId: string,
  recordId: string,
) => {
  const record = await db.vaultRecord.findFirst({
    where: { id: recordId, userId },
    select: { id: true },
  });

  if (!record) throw new ServiceError("NOT_FOUND", "Vault record not found");

  await db.vaultRecord.delete({ where: { id: recordId } });
};
