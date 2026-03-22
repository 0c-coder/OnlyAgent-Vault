"use client";

import { useEffect, useState, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@onecli/ui/card";
import { Badge } from "@onecli/ui/badge";
import { Button } from "@onecli/ui/button";
import {
  Usb,
  Square,
  Loader2,
  CheckCircle2,
  XCircle,
  Clock,
  Monitor,
} from "lucide-react";
import { toast } from "sonner";
import { OnlyKeyHands } from "@/lib/hands/webhid";
import { HandsBridge, establishHandsSession } from "@/lib/hands/bridge";
import type { JobDetail, StepStatus } from "@/lib/hands/types";

interface LiveViewProps {
  job: JobDetail;
}

const stepStatusIcon: Record<string, typeof Clock> = {
  pending: Clock,
  sending: Loader2,
  executing: Loader2,
  verifying: Loader2,
  succeeded: CheckCircle2,
  failed: XCircle,
  skipped: XCircle,
};

export const LiveView = ({ job }: LiveViewProps) => {
  const [onlyKey, setOnlyKey] = useState<OnlyKeyHands | null>(null);
  const [bridge, setBridge] = useState<HandsBridge | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [running, setRunning] = useState(false);

  // Connect OnlyKey and establish session
  const handleConnect = useCallback(async () => {
    setConnecting(true);
    try {
      const ok = await OnlyKeyHands.connect();
      setOnlyKey(ok);
      toast.success(`OnlyKey connected: ${ok.deviceInfo.productName}`);

      // Establish session
      toast.info("Press the OnlyKey button to authorize the session...");
      const { bridge: b, sessionId: sid } = await establishHandsSession(
        ok,
        job.id,
        job.host_os ?? undefined,
      );
      setBridge(b);
      setSessionId(sid);
      toast.success("Session authorized — ready to execute");
    } catch (err) {
      toast.error(
        `Connection failed: ${err instanceof Error ? err.message : "Unknown error"}`,
      );
    } finally {
      setConnecting(false);
    }
  }, [job.id, job.host_os]);

  // Start execution
  const handleStart = useCallback(async () => {
    if (!bridge) return;
    setRunning(true);
    bridge.start(); // non-blocking
    toast.success("Execution started — delivering instructions via WebHID");
  }, [bridge]);

  // Emergency stop
  const handleStop = useCallback(async () => {
    if (!bridge) return;
    try {
      await bridge.emergencyStop();
      toast.warning("Emergency stop — all execution halted");
    } catch (err) {
      toast.error("Stop failed");
    }
    setRunning(false);
  }, [bridge]);

  return (
    <div className="space-y-4">
      {/* Connection + Control Bar */}
      <Card>
        <CardContent className="flex items-center justify-between py-3">
          <div className="flex items-center gap-3">
            <div className="flex items-center gap-2">
              <Monitor className="h-4 w-4" />
              <span className="font-medium text-sm">{job.name}</span>
            </div>
            {sessionId && (
              <Badge variant="default" className="bg-green-600 text-xs">
                Session Active
              </Badge>
            )}
            {onlyKey && (
              <Badge variant="outline" className="text-xs">
                <Usb className="mr-1 h-3 w-3" />
                {onlyKey.deviceInfo.productName}
              </Badge>
            )}
          </div>
          <div className="flex items-center gap-2">
            {!onlyKey ? (
              <Button size="sm" onClick={handleConnect} disabled={connecting}>
                {connecting ? (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                ) : (
                  <Usb className="mr-2 h-4 w-4" />
                )}
                {connecting ? "Connecting..." : "Connect OnlyKey"}
              </Button>
            ) : !running ? (
              <Button size="sm" onClick={handleStart} disabled={!bridge}>
                Start Execution
              </Button>
            ) : (
              <Button
                size="sm"
                variant="destructive"
                onClick={handleStop}
              >
                <Square className="mr-2 h-4 w-4" />
                Emergency Stop
              </Button>
            )}
          </div>
        </CardContent>
      </Card>

      {/* Step List */}
      <div className="grid grid-cols-3 gap-4">
        <div className="col-span-1 space-y-2">
          <h3 className="text-sm font-medium">Steps</h3>
          {job.steps.map((step) => {
            const Icon = stepStatusIcon[step.status] ?? Clock;
            const isActive = step.status === "executing" || step.status === "sending";
            return (
              <Card
                key={step.id}
                className={isActive ? "border-blue-400" : ""}
              >
                <CardContent className="py-2 px-3">
                  <div className="flex items-center gap-2">
                    <Icon
                      className={`h-3 w-3 ${isActive ? "animate-spin text-blue-500" : "text-muted-foreground"}`}
                    />
                    <span className="text-xs">{step.description}</span>
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>

        {/* Screenshot feed placeholder */}
        <div className="col-span-2">
          <Card className="h-64 flex items-center justify-center">
            <CardContent className="text-center">
              <Monitor className="mx-auto mb-2 h-10 w-10 text-muted-foreground" />
              <p className="text-muted-foreground text-sm">
                {running
                  ? "Waiting for screenshots..."
                  : "Screenshots will appear here during execution"}
              </p>
            </CardContent>
          </Card>
        </div>
      </div>
    </div>
  );
};
