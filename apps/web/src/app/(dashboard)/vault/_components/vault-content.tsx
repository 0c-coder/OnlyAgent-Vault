"use client";

import { useEffect, useState, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@onecli/ui/card";
import { Badge } from "@onecli/ui/badge";
import { Button } from "@onecli/ui/button";
import { Shield, ShieldCheck, Lock, Unlock, Loader2, Usb } from "lucide-react";
import { toast } from "sonner";
import type { PendingApprovalRequest } from "@/lib/vault/types";
import {
  getPendingApprovals,
  submitApproval,
  lockRecord,
  revokeAllCache,
} from "@/lib/vault/api";
import {
  connectOnlyKey,
  deriveSecretForRecord,
  type OnlyKeyInstance,
} from "@/lib/vault/onlykey";
import { PendingRequestCard } from "./pending-request-card";

export const VaultContent = () => {
  const [pendingRequests, setPendingRequests] = useState<
    PendingApprovalRequest[]
  >([]);
  const [onlyKey, setOnlyKey] = useState<OnlyKeyInstance | null>(null);
  const [connecting, setConnecting] = useState(false);
  const [polling, setPolling] = useState(false);
  const [approvingId, setApprovingId] = useState<string | null>(null);

  // TODO: Get real user ID from session
  const userId = "current-user";
  const browserSessionId = `bsess_${Date.now()}`;

  // Connect to OnlyKey
  const handleConnect = useCallback(async () => {
    setConnecting(true);
    try {
      const ok = await connectOnlyKey();
      setOnlyKey(ok);
      toast.success("OnlyKey connected");
    } catch (err) {
      toast.error(
        `Failed to connect OnlyKey: ${err instanceof Error ? err.message : "Unknown error"}`,
      );
    } finally {
      setConnecting(false);
    }
  }, []);

  // Poll for pending requests
  useEffect(() => {
    if (!onlyKey) return;

    setPolling(true);
    let active = true;

    const poll = async () => {
      while (active) {
        try {
          const requests = await getPendingApprovals(userId);
          if (active) setPendingRequests(requests);
        } catch {
          // Retry silently
        }
        await new Promise((r) => setTimeout(r, 3000));
      }
    };

    poll();
    return () => {
      active = false;
      setPolling(false);
    };
  }, [onlyKey, userId]);

  // Approve a pending request using OnlyKey
  const handleApprove = useCallback(
    async (request: PendingApprovalRequest) => {
      if (!onlyKey) {
        toast.error("OnlyKey not connected");
        return;
      }

      setApprovingId(request.request_id);
      try {
        // Derive the shared secret via OnlyKey FIDO2 bridge
        const { derivedSecretB64 } = await deriveSecretForRecord({
          ok: onlyKey,
          onecliRecordPubkeyJwk: request.onecli_record_pubkey_jwk,
          additionalData: request.additional_data,
          pressRequired: true,
        });

        // Submit to gateway
        await submitApproval({
          request_id: request.request_id,
          derived_secret_b64: derivedSecretB64,
          browser_session_id: browserSessionId,
        });

        toast.success(`Approved: ${request.record_name}`);

        // Remove from pending list
        setPendingRequests((prev) =>
          prev.filter((r) => r.request_id !== request.request_id),
        );
      } catch (err) {
        toast.error(
          `Approval failed: ${err instanceof Error ? err.message : "Unknown error"}`,
        );
      } finally {
        setApprovingId(null);
      }
    },
    [onlyKey, browserSessionId],
  );

  // Lock a record manually
  const handleLock = useCallback(async (recordId: string) => {
    try {
      await lockRecord(recordId);
      toast.success("Record locked");
    } catch (err) {
      toast.error(
        `Lock failed: ${err instanceof Error ? err.message : "Unknown error"}`,
      );
    }
  }, []);

  // Emergency: revoke all
  const handleRevokeAll = useCallback(async () => {
    try {
      await revokeAllCache();
      toast.success("All cached keys revoked");
    } catch (err) {
      toast.error(
        `Revoke failed: ${err instanceof Error ? err.message : "Unknown error"}`,
      );
    }
  }, []);

  return (
    <div className="space-y-6">
      {/* OnlyKey Connection Status */}
      <Card>
        <CardHeader className="flex flex-row items-center justify-between pb-2">
          <CardTitle className="text-sm font-medium">
            OnlyKey Connection
          </CardTitle>
          {onlyKey ? (
            <Badge variant="default" className="bg-green-600">
              <ShieldCheck className="mr-1 h-3 w-3" />
              Connected
            </Badge>
          ) : (
            <Badge variant="secondary">
              <Shield className="mr-1 h-3 w-3" />
              Disconnected
            </Badge>
          )}
        </CardHeader>
        <CardContent>
          {!onlyKey ? (
            <div className="flex items-center gap-4">
              <p className="text-muted-foreground text-sm">
                Connect your OnlyKey to approve vault requests from agents.
              </p>
              <Button
                onClick={handleConnect}
                disabled={connecting}
                size="sm"
              >
                {connecting ? (
                  <Loader2 className="mr-2 h-4 w-4 animate-spin" />
                ) : (
                  <Usb className="mr-2 h-4 w-4" />
                )}
                {connecting ? "Connecting..." : "Connect OnlyKey"}
              </Button>
            </div>
          ) : (
            <div className="flex items-center justify-between">
              <p className="text-muted-foreground text-sm">
                {polling
                  ? "Listening for approval requests..."
                  : "OnlyKey ready."}
              </p>
              <Button
                variant="destructive"
                size="sm"
                onClick={handleRevokeAll}
              >
                <Lock className="mr-2 h-4 w-4" />
                Lock All Records
              </Button>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Pending Approval Requests */}
      {onlyKey && (
        <div className="space-y-3">
          <h2 className="text-lg font-medium">Pending Approvals</h2>
          {pendingRequests.length === 0 ? (
            <Card>
              <CardContent className="py-8 text-center">
                <Unlock className="mx-auto mb-2 h-8 w-8 text-muted-foreground" />
                <p className="text-muted-foreground text-sm">
                  No pending approval requests. Agents will appear here when
                  they need access to vault-protected secrets.
                </p>
              </CardContent>
            </Card>
          ) : (
            pendingRequests.map((request) => (
              <PendingRequestCard
                key={request.request_id}
                request={request}
                onApprove={handleApprove}
                onLock={handleLock}
                approving={approvingId === request.request_id}
              />
            ))
          )}
        </div>
      )}
    </div>
  );
};
