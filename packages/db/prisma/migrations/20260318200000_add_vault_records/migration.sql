-- CreateTable
CREATE TABLE "VaultRecord" (
    "id" TEXT NOT NULL,
    "userId" TEXT NOT NULL,
    "name" TEXT NOT NULL,
    "recordType" TEXT NOT NULL,
    "ciphertextB64" TEXT NOT NULL,
    "nonceB64" TEXT NOT NULL,
    "aadJson" TEXT NOT NULL,
    "wrappedKeyB64" TEXT NOT NULL,
    "wrappedKeyNonceB64" TEXT NOT NULL,
    "wrapAlg" TEXT NOT NULL DEFAULT 'OK-DERIVED-HKDF-AESGCM-v1',
    "onecliPubkeyJwk" TEXT NOT NULL,
    "derivationContext" TEXT NOT NULL,
    "requireOnlykey" BOOLEAN NOT NULL DEFAULT true,
    "unlockTtlSeconds" INTEGER NOT NULL DEFAULT 86400,
    "idleTimeoutSeconds" INTEGER NOT NULL DEFAULT 3600,
    "cacheScope" TEXT NOT NULL DEFAULT 'agent',
    "allowManualRevoke" BOOLEAN NOT NULL DEFAULT true,
    "relockOnBrowserDisconnect" BOOLEAN NOT NULL DEFAULT true,
    "relockOnPolicyChange" BOOLEAN NOT NULL DEFAULT true,
    "requireFreshUnlockForHighRisk" BOOLEAN NOT NULL DEFAULT true,
    "allowPlaintextReturn" BOOLEAN NOT NULL DEFAULT true,
    "allowedAgents" TEXT,
    "recordVersion" INTEGER NOT NULL DEFAULT 1,
    "policyVersion" INTEGER NOT NULL DEFAULT 1,
    "keyVersion" INTEGER NOT NULL DEFAULT 1,
    "unlockGeneration" INTEGER NOT NULL DEFAULT 1,
    "hostPattern" TEXT NOT NULL,
    "pathPattern" TEXT,
    "injectionConfig" JSONB,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "updatedAt" TIMESTAMP(3) NOT NULL,

    CONSTRAINT "VaultRecord_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "VaultApprovalRequest" (
    "id" TEXT NOT NULL,
    "recordId" TEXT NOT NULL,
    "agentId" TEXT NOT NULL,
    "sessionId" TEXT,
    "operation" TEXT NOT NULL,
    "status" TEXT NOT NULL DEFAULT 'pending',
    "origin" TEXT NOT NULL,
    "nonceB64" TEXT NOT NULL,
    "browserSessionId" TEXT,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,
    "expiresAt" TIMESTAMP(3) NOT NULL,

    CONSTRAINT "VaultApprovalRequest_pkey" PRIMARY KEY ("id")
);

-- CreateTable
CREATE TABLE "VaultAuditEvent" (
    "id" TEXT NOT NULL,
    "recordId" TEXT NOT NULL,
    "event" TEXT NOT NULL,
    "agentId" TEXT,
    "sessionId" TEXT,
    "scopeType" TEXT,
    "scopeId" TEXT,
    "reason" TEXT,
    "metadata" JSONB,
    "createdAt" TIMESTAMP(3) NOT NULL DEFAULT CURRENT_TIMESTAMP,

    CONSTRAINT "VaultAuditEvent_pkey" PRIMARY KEY ("id")
);

-- CreateIndex
CREATE INDEX "VaultRecord_userId_idx" ON "VaultRecord"("userId");

-- CreateIndex
CREATE INDEX "VaultApprovalRequest_recordId_idx" ON "VaultApprovalRequest"("recordId");

-- CreateIndex
CREATE INDEX "VaultApprovalRequest_status_idx" ON "VaultApprovalRequest"("status");

-- CreateIndex
CREATE INDEX "VaultAuditEvent_recordId_idx" ON "VaultAuditEvent"("recordId");

-- CreateIndex
CREATE INDEX "VaultAuditEvent_event_idx" ON "VaultAuditEvent"("event");

-- CreateIndex
CREATE INDEX "VaultAuditEvent_createdAt_idx" ON "VaultAuditEvent"("createdAt");

-- AddForeignKey
ALTER TABLE "VaultRecord" ADD CONSTRAINT "VaultRecord_userId_fkey" FOREIGN KEY ("userId") REFERENCES "User"("id") ON DELETE RESTRICT ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE "VaultApprovalRequest" ADD CONSTRAINT "VaultApprovalRequest_recordId_fkey" FOREIGN KEY ("recordId") REFERENCES "VaultRecord"("id") ON DELETE CASCADE ON UPDATE CASCADE;

-- AddForeignKey
ALTER TABLE "VaultAuditEvent" ADD CONSTRAINT "VaultAuditEvent_recordId_fkey" FOREIGN KEY ("recordId") REFERENCES "VaultRecord"("id") ON DELETE CASCADE ON UPDATE CASCADE;
