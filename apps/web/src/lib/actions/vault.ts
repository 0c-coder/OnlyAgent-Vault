"use server";

import { resolveUserId } from "@/lib/actions/resolve-user";
import {
  listVaultRecords,
  createVaultRecord as createVaultRecordService,
  deleteVaultRecord as deleteVaultRecordService,
  type CreateVaultRecordData,
} from "@/lib/services/vault-record-service";

export const getVaultRecords = async () => {
  const userId = await resolveUserId();
  return listVaultRecords(userId);
};

export const createVaultRecord = async (data: CreateVaultRecordData) => {
  const userId = await resolveUserId();
  return createVaultRecordService(userId, data);
};

export const deleteVaultRecord = async (recordId: string) => {
  const userId = await resolveUserId();
  return deleteVaultRecordService(userId, recordId);
};
