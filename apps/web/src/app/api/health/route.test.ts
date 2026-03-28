import { describe, it, expect, vi, beforeEach } from "vitest";

vi.mock("@onecli/db", () => ({
  db: {
    $queryRaw: vi.fn(),
  },
}));

// Dynamic import after mock registration
const importRoute = () => import("./route");

describe("GET /api/health", () => {
  beforeEach(() => {
    vi.resetModules();
  });

  it("returns 200 with ok status when database is healthy", async () => {
    const { db } = await import("@onecli/db");
    vi.mocked(db.$queryRaw).mockResolvedValueOnce([{ "?column?": 1 }]);

    const { GET } = await importRoute();
    const response = await GET();
    const body = await response.json();

    expect(response.status).toBe(200);
    expect(body.status).toBe("ok");
    expect(body.checks.database.status).toBe("ok");
    expect(typeof body.checks.database.latencyMs).toBe("number");
    expect(typeof body.timestamp).toBe("string");
  });

  it("returns 503 with degraded status when database is unreachable", async () => {
    const { db } = await import("@onecli/db");
    vi.mocked(db.$queryRaw).mockRejectedValueOnce(new Error("Connection refused"));

    const { GET } = await importRoute();
    const response = await GET();
    const body = await response.json();

    expect(response.status).toBe(503);
    expect(body.status).toBe("degraded");
    expect(body.checks.database.status).toBe("error");
    expect(body.checks.database.error).toBe("Connection refused");
    expect(typeof body.checks.database.latencyMs).toBe("number");
  });

  it("returns 503 with degraded status when database throws unknown error", async () => {
    const { db } = await import("@onecli/db");
    vi.mocked(db.$queryRaw).mockRejectedValueOnce("non-error thrown");

    const { GET } = await importRoute();
    const response = await GET();
    const body = await response.json();

    expect(response.status).toBe(503);
    expect(body.status).toBe("degraded");
    expect(body.checks.database.error).toBe("Unknown database error");
  });

  it("response includes a valid ISO 8601 timestamp", async () => {
    const { db } = await import("@onecli/db");
    vi.mocked(db.$queryRaw).mockResolvedValueOnce([]);

    const { GET } = await importRoute();
    const response = await GET();
    const body = await response.json();

    expect(new Date(body.timestamp).toISOString()).toBe(body.timestamp);
  });

  it("database latency is non-negative", async () => {
    const { db } = await import("@onecli/db");
    vi.mocked(db.$queryRaw).mockResolvedValueOnce([]);

    const { GET } = await importRoute();
    const response = await GET();
    const body = await response.json();

    expect(body.checks.database.latencyMs).toBeGreaterThanOrEqual(0);
  });
});
