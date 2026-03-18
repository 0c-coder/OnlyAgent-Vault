"use client";

import { Card, CardContent } from "@onecli/ui/card";
import { Badge } from "@onecli/ui/badge";
import { Button } from "@onecli/ui/button";
import { ShieldCheck, Lock, Loader2, Clock, Bot } from "lucide-react";
import type { PendingApprovalRequest } from "@/lib/vault/types";

interface PendingRequestCardProps {
  request: PendingApprovalRequest;
  onApprove: (request: PendingApprovalRequest) => void;
  onLock: (recordId: string) => void;
  approving: boolean;
}

export const PendingRequestCard = ({
  request,
  onApprove,
  onLock,
  approving,
}: PendingRequestCardProps) => {
  const expiresAt = new Date(request.expires_at);
  const now = new Date();
  const secondsRemaining = Math.max(
    0,
    Math.floor((expiresAt.getTime() - now.getTime()) / 1000),
  );

  const operationLabel: Record<string, string> = {
    unlock_record_key: "Unlock Secret",
    decrypt_age: "Decrypt Payload",
    sign_blob: "Sign Data",
  };

  return (
    <Card className="border-amber-200 dark:border-amber-800">
      <CardContent className="flex items-center justify-between py-4">
        <div className="flex flex-col gap-1">
          <div className="flex items-center gap-2">
            <span className="font-medium">{request.record_name}</span>
            <Badge variant="outline" className="text-xs">
              {operationLabel[request.operation] ?? request.operation}
            </Badge>
          </div>
          <div className="text-muted-foreground flex items-center gap-3 text-xs">
            <span className="flex items-center gap-1">
              <Bot className="h-3 w-3" />
              {request.agent_id}
            </span>
            <span className="flex items-center gap-1">
              <Clock className="h-3 w-3" />
              {secondsRemaining > 0
                ? `${secondsRemaining}s remaining`
                : "Expired"}
            </span>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => onLock(request.record_id)}
          >
            <Lock className="mr-1 h-3 w-3" />
            Deny
          </Button>
          <Button
            size="sm"
            onClick={() => onApprove(request)}
            disabled={approving || secondsRemaining === 0}
          >
            {approving ? (
              <Loader2 className="mr-1 h-3 w-3 animate-spin" />
            ) : (
              <ShieldCheck className="mr-1 h-3 w-3" />
            )}
            {approving ? "Touch OnlyKey..." : "Approve"}
          </Button>
        </div>
      </CardContent>
    </Card>
  );
};
