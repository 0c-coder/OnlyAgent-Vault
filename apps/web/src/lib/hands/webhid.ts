/**
 * WebHID driver for OnlyKey.
 *
 * Provides direct USB HID communication with an OnlyKey device
 * from the browser using the WebHID API (Chrome/Edge 89+).
 */

export const ONLYKEY_VENDOR_ID = 0x16c0; // Teensy/PJRC

// ── Hands protocol report IDs ──────────────────────────────────────────

const REPORT_ID_INSTRUCTION = 0x70;
const REPORT_ID_SESSION_AUTH = 0x71;
// const REPORT_ID_ACK = 0x72;           // device → browser
const REPORT_ID_STATUS = 0x73; // device → browser
const REPORT_ID_EMERGENCY_STOP = 0x74;
// const REPORT_ID_PING = 0x75;

// ── Types ──────────────────────────────────────────────────────────────

export type DeviceStatus =
  | { type: "queued"; seqNo: number }
  | { type: "executing"; seqNo: number }
  | { type: "complete"; seqNo: number }
  | { type: "error"; seqNo: number; detail: string }
  | { type: "button_stop" };

type StatusHandler = (status: DeviceStatus) => void;

// ── OnlyKeyHands class ─────────────────────────────────────────────────

export class OnlyKeyHands {
  private device: HIDDevice;
  private statusHandler: StatusHandler | null = null;

  constructor(device: HIDDevice) {
    this.device = device;
    this.device.addEventListener(
      "inputreport",
      this.handleReport.bind(this) as EventListener,
    );
  }

  /**
   * Request and open the OnlyKey device via WebHID.
   * Requires a user gesture (button click) to trigger the device picker.
   */
  static async connect(): Promise<OnlyKeyHands> {
    if (!("hid" in navigator)) {
      throw new Error(
        "WebHID is not available in this browser. Please use Chrome or Edge.",
      );
    }

    const devices = await navigator.hid.requestDevice({
      filters: [{ vendorId: ONLYKEY_VENDOR_ID }],
    });

    if (devices.length === 0) {
      throw new Error("No OnlyKey device selected.");
    }

    const device = devices[0]!;
    await device.open();
    return new OnlyKeyHands(device);
  }

  /**
   * Send session auth report to OnlyKey.
   * Device will flash; user must press the button to authorize.
   */
  async authorizeSession(
    sessionId: string,
    nonce: Uint8Array,
  ): Promise<void> {
    const payload = new Uint8Array(64);
    const encoder = new TextEncoder();
    const sidBytes = encoder.encode(
      sessionId.replace(/-/g, "").slice(0, 16),
    );
    payload.set(sidBytes, 0);
    payload.set(nonce.slice(0, 16), 16);
    await this.device.sendReport(REPORT_ID_SESSION_AUTH, payload);
  }

  /**
   * Send a single HID instruction report (one chunk of a larger packet).
   */
  async sendInstructionReport(
    seqNo: number,
    total: number,
    flags: number,
    payloadSlice: Uint8Array,
  ): Promise<void> {
    const report = new Uint8Array(64);
    report[0] = (seqNo >> 8) & 0xff;
    report[1] = seqNo & 0xff;
    report[2] = (total >> 8) & 0xff;
    report[3] = total & 0xff;
    report[4] = flags;
    report.set(payloadSlice.slice(0, 59), 5);
    await this.device.sendReport(REPORT_ID_INSTRUCTION, report);
  }

  /**
   * Deliver a complete compiled instruction set (CBOR bytes)
   * by chunking into 59-byte HID report payloads.
   */
  async deliverInstructionSet(
    cbor: Uint8Array,
    flags: number = 0x01,
  ): Promise<void> {
    const PAYLOAD_SIZE = 59;
    const total = Math.ceil(cbor.length / PAYLOAD_SIZE);

    for (let seqNo = 0; seqNo < total; seqNo++) {
      const slice = cbor.slice(
        seqNo * PAYLOAD_SIZE,
        (seqNo + 1) * PAYLOAD_SIZE,
      );
      await this.sendInstructionReport(seqNo, total, flags, slice);
      // Brief pause between reports to avoid overwhelming HID queue
      if (seqNo < total - 1) {
        await sleep(10);
      }
    }
  }

  /** Immediately halt OnlyKey execution. */
  async emergencyStop(): Promise<void> {
    const payload = new Uint8Array(64);
    payload[0] = 0xff;
    await this.device.sendReport(REPORT_ID_EMERGENCY_STOP, payload);
  }

  /** Register a handler for device status reports. */
  onStatusUpdate(handler: StatusHandler): void {
    this.statusHandler = handler;
  }

  /** Close the HID device connection. */
  async close(): Promise<void> {
    await this.device.close();
  }

  get isConnected(): boolean {
    return this.device.opened;
  }

  get deviceInfo(): { productName: string; vendorId: number } {
    return {
      productName: this.device.productName ?? "OnlyKey",
      vendorId: this.device.vendorId,
    };
  }

  // ── Internal ───────────────────────────────────────────────────────

  private handleReport(event: HIDInputReportEvent): void {
    const data = new Uint8Array(event.data.buffer);

    if (event.reportId === REPORT_ID_STATUS) {
      const seqNo = (data[0]! << 8) | data[1]!;
      const statusCode = data[2]!;
      const detail = new TextDecoder()
        .decode(data.slice(3))
        .replace(/\0/g, "");

      const statusMap: Record<number, DeviceStatus["type"]> = {
        0x00: "queued",
        0x01: "executing",
        0x02: "complete",
        0x03: "error",
        0x04: "button_stop",
      };

      const type = statusMap[statusCode] ?? "error";

      if (type === "button_stop") {
        this.statusHandler?.({ type: "button_stop" });
      } else {
        this.statusHandler?.({
          type,
          seqNo,
          detail,
        } as DeviceStatus);
      }
    }
  }
}

// ── Helpers ─────────────────────────────────────────────────────────────

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
