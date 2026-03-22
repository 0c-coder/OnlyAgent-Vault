/**
 * Client-side API for the OnlyAgent Hands gateway endpoints.
 *
 * These functions call the gateway's hands endpoints from the browser.
 * The browser acts as a relay between OnlyKey (WebHID) and the gateway.
 */

import type {
  CreateJobInput,
  CreateSessionResponse,
  JobDetail,
  JobSummary,
  NextPacketResponse,
  SessionView,
  StepStatusReport,
} from "./types";

// ── Configuration ───────────────────────────────────────────────────────

const getGatewayBaseUrl = (): string => {
  if (typeof window !== "undefined") {
    return (
      (window as unknown as Record<string, string>).__GATEWAY_URL__ ??
      process.env.NEXT_PUBLIC_GATEWAY_URL ??
      "https://localhost:10255"
    );
  }
  return process.env.GATEWAY_URL ?? "https://localhost:10255";
};

// ── Job API ─────────────────────────────────────────────────────────────

export const createJob = async (
  input: CreateJobInput,
): Promise<{ id: string; status: string }> => {
  const res = await fetch(`${getGatewayBaseUrl()}/v1/hands/jobs`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(input),
  });
  if (!res.ok) throw new Error(`Failed to create job: ${res.status}`);
  return res.json();
};

export const listJobs = async (): Promise<JobSummary[]> => {
  const res = await fetch(`${getGatewayBaseUrl()}/v1/hands/jobs`, {
    credentials: "include",
  });
  if (!res.ok) throw new Error(`Failed to list jobs: ${res.status}`);
  const data = await res.json();
  return data.items;
};

export const getJob = async (jobId: string): Promise<JobDetail> => {
  const res = await fetch(
    `${getGatewayBaseUrl()}/v1/hands/jobs/${encodeURIComponent(jobId)}`,
    { credentials: "include" },
  );
  if (!res.ok) throw new Error(`Failed to get job: ${res.status}`);
  return res.json();
};

export const startJob = async (jobId: string): Promise<void> => {
  const res = await fetch(
    `${getGatewayBaseUrl()}/v1/hands/jobs/${encodeURIComponent(jobId)}/start`,
    { method: "POST", credentials: "include" },
  );
  if (!res.ok) throw new Error(`Failed to start job: ${res.status}`);
};

export const cancelJob = async (jobId: string): Promise<void> => {
  const res = await fetch(
    `${getGatewayBaseUrl()}/v1/hands/jobs/${encodeURIComponent(jobId)}/cancel`,
    { method: "POST", credentials: "include" },
  );
  if (!res.ok) throw new Error(`Failed to cancel job: ${res.status}`);
};

// ── Session API ─────────────────────────────────────────────────────────

export const createSession = async (
  jobId: string,
  hostOS?: string,
): Promise<CreateSessionResponse> => {
  const res = await fetch(`${getGatewayBaseUrl()}/v1/hands/sessions`, {
    method: "POST",
    credentials: "include",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ job_id: jobId, host_os: hostOS }),
  });
  if (!res.ok) throw new Error(`Failed to create session: ${res.status}`);
  return res.json();
};

export const getSession = async (
  sessionId: string,
): Promise<SessionView> => {
  const res = await fetch(
    `${getGatewayBaseUrl()}/v1/hands/sessions/${encodeURIComponent(sessionId)}`,
    { credentials: "include" },
  );
  if (!res.ok) throw new Error(`Failed to get session: ${res.status}`);
  return res.json();
};

export const closeSession = async (sessionId: string): Promise<void> => {
  await fetch(
    `${getGatewayBaseUrl()}/v1/hands/sessions/${encodeURIComponent(sessionId)}`,
    { method: "DELETE", credentials: "include" },
  );
};

export const activateSession = async (
  sessionId: string,
  browserSessionId: string,
  deviceId?: string,
  hostOS?: string,
): Promise<void> => {
  const res = await fetch(
    `${getGatewayBaseUrl()}/v1/hands/sessions/${encodeURIComponent(sessionId)}/activated`,
    {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        browser_session_id: browserSessionId,
        device_id: deviceId,
        host_os: hostOS,
      }),
    },
  );
  if (!res.ok) throw new Error(`Failed to activate session: ${res.status}`);
};

export const emergencyStop = async (sessionId: string): Promise<void> => {
  await fetch(
    `${getGatewayBaseUrl()}/v1/hands/sessions/${encodeURIComponent(sessionId)}/emergency-stop`,
    { method: "POST", credentials: "include" },
  );
};

// ── Instruction delivery API ────────────────────────────────────────────

export const fetchNextPacket = async (
  sessionId: string,
): Promise<NextPacketResponse | null> => {
  const res = await fetch(
    `${getGatewayBaseUrl()}/v1/hands/sessions/${encodeURIComponent(sessionId)}/next-packet`,
    { credentials: "include" },
  );
  if (res.status === 204) return null;
  if (!res.ok) throw new Error(`Failed to fetch packet: ${res.status}`);
  return res.json();
};

export const reportPacketAck = async (
  sessionId: string,
  packetId: string,
): Promise<void> => {
  await fetch(
    `${getGatewayBaseUrl()}/v1/hands/sessions/${encodeURIComponent(sessionId)}/packet-acked`,
    {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ packet_id: packetId }),
    },
  );
};

export const reportStepStatus = async (
  sessionId: string,
  report: StepStatusReport,
): Promise<void> => {
  await fetch(
    `${getGatewayBaseUrl()}/v1/hands/sessions/${encodeURIComponent(sessionId)}/step-status`,
    {
      method: "POST",
      credentials: "include",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(report),
    },
  );
};
