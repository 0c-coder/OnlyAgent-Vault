/**
 * Client-side API for the OnlyKey Vault browser bridge.
 *
 * These functions call the gateway's vault endpoints from the browser.
 * The browser is the trusted intermediary between OnlyKey (hardware)
 * and the gateway (server).
 */

import type {
  BrowserApprovePayload,
  BrowserApproveResponse,
  PendingApprovalRequest,
  PendingRequestsResponse,
} from "./types";

// ── Configuration ───────────────────────────────────────────────────────

/**
 * Get the gateway base URL. In development this is localhost:10255,
 * in production it's the Cloudflare Tunnel URL.
 */
const getGatewayBaseUrl = (): string => {
  if (typeof window !== "undefined") {
    // Use the current origin if running on the same domain as the gateway
    return (
      (window as unknown as Record<string, string>).__GATEWAY_URL__ ??
      process.env.NEXT_PUBLIC_GATEWAY_URL ??
      "https://localhost:10255"
    );
  }
  return process.env.GATEWAY_URL ?? "https://localhost:10255";
};

// ── Browser → Gateway API calls ─────────────────────────────────────────

/**
 * Poll for pending approval requests that need OnlyKey fulfillment.
 */
export const getPendingApprovals = async (
  userId: string,
): Promise<PendingApprovalRequest[]> => {
  const baseUrl = getGatewayBaseUrl();
  const res = await fetch(
    `${baseUrl}/v1/vault/browser/pending?user_id=${encodeURIComponent(userId)}`,
    {
      credentials: "include",
      headers: { Accept: "application/json" },
    },
  );

  if (!res.ok) {
    throw new Error(`Failed to fetch pending approvals: ${res.status}`);
  }

  const data: PendingRequestsResponse = await res.json();
  return data.items;
};

/**
 * Submit an OnlyKey-derived approval to the gateway.
 *
 * After the browser derives the shared secret via OnlyKey, it sends
 * the result here. The gateway uses it to unwrap the record key.
 */
export const submitApproval = async (
  payload: BrowserApprovePayload,
): Promise<BrowserApproveResponse> => {
  const baseUrl = getGatewayBaseUrl();
  const res = await fetch(`${baseUrl}/v1/vault/browser/approve`, {
    method: "POST",
    credentials: "include",
    headers: {
      "Content-Type": "application/json",
      Accept: "application/json",
    },
    body: JSON.stringify(payload),
  });

  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(
      (body as { message?: string }).message ??
        `Approval failed: ${res.status}`,
    );
  }

  return res.json();
};

/**
 * Manually lock a specific vault record.
 */
export const lockRecord = async (recordId: string): Promise<void> => {
  const baseUrl = getGatewayBaseUrl();
  const res = await fetch(
    `${baseUrl}/v1/vault/records/${encodeURIComponent(recordId)}/lock`,
    {
      method: "POST",
      credentials: "include",
    },
  );

  if (!res.ok) {
    throw new Error(`Failed to lock record: ${res.status}`);
  }
};

/**
 * Lock all vault records for a specific agent.
 */
export const lockAgentRecords = async (agentId: string): Promise<void> => {
  const baseUrl = getGatewayBaseUrl();
  const res = await fetch(
    `${baseUrl}/v1/vault/agents/${encodeURIComponent(agentId)}/lock`,
    {
      method: "POST",
      credentials: "include",
    },
  );

  if (!res.ok) {
    throw new Error(`Failed to lock agent records: ${res.status}`);
  }
};

/**
 * Revoke all cached vault keys (admin emergency action).
 */
export const revokeAllCache = async (): Promise<void> => {
  const baseUrl = getGatewayBaseUrl();
  const res = await fetch(`${baseUrl}/v1/vault/cache/revoke-all`, {
    method: "POST",
    credentials: "include",
  });

  if (!res.ok) {
    throw new Error(`Failed to revoke cache: ${res.status}`);
  }
};

// ── Polling helper ──────────────────────────────────────────────────────

/**
 * Start polling for pending approval requests.
 * Returns a cleanup function to stop polling.
 */
export const startPolling = (
  userId: string,
  onRequests: (requests: PendingApprovalRequest[]) => void,
  intervalMs: number = 3000,
): (() => void) => {
  let active = true;

  const poll = async () => {
    while (active) {
      try {
        const requests = await getPendingApprovals(userId);
        if (active) {
          onRequests(requests);
        }
      } catch {
        // Silently retry on error
      }
      await new Promise((resolve) => setTimeout(resolve, intervalMs));
    }
  };

  poll();

  return () => {
    active = false;
  };
};
