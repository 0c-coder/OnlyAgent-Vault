import { Suspense } from "react";
import type { Metadata } from "next";
import { PageHeader } from "@dashboard/page-header";
import { HandsJobList } from "./_components/job-list";

export const metadata: Metadata = {
  title: "Hands",
};

export default function HandsPage() {
  return (
    <div className="flex flex-1 flex-col gap-6 max-w-5xl">
      <PageHeader
        title="OnlyAgent Hands"
        description="Remote machine control via OnlyKey. Send keystrokes through WebHID, capture screenshots, and let AI reason about the results."
      />
      <Suspense>
        <HandsJobList />
      </Suspense>
    </div>
  );
}
