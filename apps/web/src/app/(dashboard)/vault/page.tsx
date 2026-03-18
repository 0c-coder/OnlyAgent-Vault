import { Suspense } from "react";
import type { Metadata } from "next";
import { PageHeader } from "@dashboard/page-header";
import { VaultContent } from "./_components/vault-content";

export const metadata: Metadata = {
  title: "Vault",
};

export default function VaultPage() {
  return (
    <div className="flex flex-1 flex-col gap-6 max-w-5xl">
      <PageHeader
        title="OnlyKey Vault"
        description="Hardware-protected secrets. Encrypted with OnlyKey — no private keys stored on server."
      />
      <Suspense>
        <VaultContent />
      </Suspense>
    </div>
  );
}
