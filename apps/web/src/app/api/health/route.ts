import { db } from "@onecli/db";
import { NextResponse } from "next/server";

type HealthStatus = "ok" | "degraded" | "error";

interface HealthCheckResponse {
  status: HealthStatus;
  timestamp: string;
  checks: {
    database: {
      status: HealthStatus;
      latencyMs?: number;
      error?: string;
    };
  };
}

const checkDatabase = async (): Promise<HealthCheckResponse["checks"]["database"]> => {
  const start = Date.now();
  try {
    await db.$queryRaw`SELECT 1`;
    return { status: "ok", latencyMs: Date.now() - start };
  } catch (err) {
    return {
      status: "error",
      latencyMs: Date.now() - start,
      error: err instanceof Error ? err.message : "Unknown database error",
    };
  }
};

export const GET = async (): Promise<NextResponse<HealthCheckResponse>> => {
  const database = await checkDatabase();

  const overallStatus: HealthStatus =
    database.status === "error" ? "degraded" : "ok";

  const httpStatus = overallStatus === "ok" ? 200 : 503;

  return NextResponse.json(
    {
      status: overallStatus,
      timestamp: new Date().toISOString(),
      checks: { database },
    },
    { status: httpStatus },
  );
};
