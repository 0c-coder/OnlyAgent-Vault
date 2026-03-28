/**
 * Hands bridge — relays instruction packets from the gateway to OnlyKey
 * via WebHID and forwards device status back to the gateway.
 */

import {
  activateSession,
  createSession,
  emergencyStop as apiEmergencyStop,
  fetchNextPacket,
  reportPacketAck,
  reportStepStatus,
} from "./api";
import type { DeviceStatus, OnlyKeyHands } from "./webhid";

// ── HandsBridge ─────────────────────────────────────────────────────────

export class HandsBridge {
  private ok: OnlyKeyHands;
  private sessionId: string;
  private gatewayUrl: string;
  private running = false;

  constructor(
    ok: OnlyKeyHands,
    sessionId: string,
    gatewayUrl: string,
  ) {
    this.ok = ok;
    this.sessionId = sessionId;
    this.gatewayUrl = gatewayUrl;

    // Forward device status to gateway
    this.ok.onStatusUpdate(async (status: DeviceStatus) => {
      if (status.type === "complete" || status.type === "error") {
        const statusCode =
          status.type === "complete" ? 0x02 : 0x03;
        await reportStepStatus(this.sessionId, {
          step_id: "", // TODO: track current step
          status_code: statusCode,
          detail:
            status.type === "error" && "detail" in status
              ? status.detail
              : undefined,
        });
      }
      if (status.type === "button_stop") {
        await apiEmergencyStop(this.sessionId);
        this.running = false;
      }
    });
  }

  /** Start the relay loop: poll gateway → deliver to OnlyKey → repeat */
  async start(): Promise<void> {
    this.running = true;

    while (this.running) {
      try {
        const packet = await fetchNextPacket(this.sessionId);

        if (packet) {
          // Decode base64 CBOR and deliver via WebHID
          const cbor = Uint8Array.from(atob(packet.cbor_b64), (c) =>
            c.charCodeAt(0),
          );
          await this.ok.deliverInstructionSet(cbor, packet.flags);
          await reportPacketAck(this.sessionId, packet.packet_id);
        } else {
          // No packet queued — wait before polling again
          await sleep(500);
        }
      } catch (err) {
        console.error("Hands bridge error:", err);
        await sleep(1000);
      }
    }
  }

  /** Stop the relay loop. */
  stop(): void {
    this.running = false;
  }

  /** Emergency stop: halt device + close session. */
  async emergencyStop(): Promise<void> {
    await this.ok.emergencyStop();
    await apiEmergencyStop(this.sessionId);
    this.running = false;
  }

  get isRunning(): boolean {
    return this.running;
  }
}

// ── Session establishment helper ────────────────────────────────────────

/**
 * Full session establishment flow:
 * 1. Connect OnlyKey via WebHID
 * 2. Create session on gateway
 * 3. Send HANDS_SESSION_AUTH to device (user presses button)
 * 4. Report activation to gateway
 * 5. Return ready-to-use bridge
 */
export async function establishHandsSession(
  ok: OnlyKeyHands,
  jobId: string,
  hostOS?: string,
): Promise<{ bridge: HandsBridge; sessionId: string; agentToken: string }> {
  // 1. Create session on gateway
  const { session_id: sessionId, nonce, agent_token: agentToken } =
    await createSession(jobId, hostOS);

  // 2. Send session auth to OnlyKey (device flashes, user presses button)
  const nonceBytes = Uint8Array.from(atob(nonce), (c) => c.charCodeAt(0));
  await ok.authorizeSession(sessionId, nonceBytes);

  // 3. Wait briefly for user to press the button
  await sleep(2000);

  // 4. Report activation to gateway
  const browserSessionId = crypto.randomUUID();
  await activateSession(
    sessionId,
    browserSessionId,
    ok.deviceInfo.productName,
    hostOS,
  );

  // 5. Create and return bridge
  const gatewayUrl =
    (window as unknown as Record<string, string>).__GATEWAY_URL__ ??
    process.env.NEXT_PUBLIC_GATEWAY_URL ??
    "https://localhost:10255";

  const bridge = new HandsBridge(ok, sessionId, gatewayUrl);

  return { bridge, sessionId, agentToken };
}

// ── Helpers ─────────────────────────────────────────────────────────────

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
