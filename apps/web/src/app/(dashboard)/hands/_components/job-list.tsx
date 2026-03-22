"use client";

import { useEffect, useState, useCallback } from "react";
import { Card, CardContent, CardHeader, CardTitle } from "@onecli/ui/card";
import { Badge } from "@onecli/ui/badge";
import { Button } from "@onecli/ui/button";
import {
  Hand,
  Plus,
  Play,
  XCircle,
  Clock,
  CheckCircle2,
  Loader2,
  AlertCircle,
} from "lucide-react";
import { toast } from "sonner";
import { listJobs, startJob, cancelJob } from "@/lib/hands/api";
import type { JobSummary, JobStatus } from "@/lib/hands/types";

const statusConfig: Record<
  JobStatus,
  { label: string; variant: "default" | "secondary" | "destructive" | "outline"; icon: typeof Clock }
> = {
  draft: { label: "Draft", variant: "secondary", icon: Clock },
  queued: { label: "Queued", variant: "outline", icon: Clock },
  running: { label: "Running", variant: "default", icon: Loader2 },
  paused: { label: "Paused", variant: "outline", icon: AlertCircle },
  completed: { label: "Completed", variant: "default", icon: CheckCircle2 },
  failed: { label: "Failed", variant: "destructive", icon: XCircle },
  cancelled: { label: "Cancelled", variant: "secondary", icon: XCircle },
};

export const HandsJobList = () => {
  const [jobs, setJobs] = useState<JobSummary[]>([]);
  const [loading, setLoading] = useState(true);

  const fetchJobs = useCallback(async () => {
    try {
      const items = await listJobs();
      setJobs(items);
    } catch {
      // silently retry
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    fetchJobs();
    const interval = setInterval(fetchJobs, 5000);
    return () => clearInterval(interval);
  }, [fetchJobs]);

  const handleStart = async (jobId: string) => {
    try {
      await startJob(jobId);
      toast.success("Job started");
      fetchJobs();
    } catch (err) {
      toast.error(`Failed to start: ${err instanceof Error ? err.message : "Unknown error"}`);
    }
  };

  const handleCancel = async (jobId: string) => {
    try {
      await cancelJob(jobId);
      toast.success("Job cancelled");
      fetchJobs();
    } catch (err) {
      toast.error(`Failed to cancel: ${err instanceof Error ? err.message : "Unknown error"}`);
    }
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center py-12">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-medium">Jobs</h2>
        <Button size="sm" asChild>
          <a href="/hands/new">
            <Plus className="mr-2 h-4 w-4" />
            New Job
          </a>
        </Button>
      </div>

      {jobs.length === 0 ? (
        <Card>
          <CardContent className="py-12 text-center">
            <Hand className="mx-auto mb-3 h-10 w-10 text-muted-foreground" />
            <p className="text-muted-foreground text-sm">
              No jobs yet. Create a new job to start controlling a remote machine
              via OnlyKey.
            </p>
          </CardContent>
        </Card>
      ) : (
        jobs.map((job) => {
          const config = statusConfig[job.status] ?? statusConfig.draft;
          const StatusIcon = config.icon;
          return (
            <Card key={job.id}>
              <CardContent className="flex items-center justify-between py-4">
                <div className="flex flex-col gap-1">
                  <div className="flex items-center gap-2">
                    <a
                      href={`/hands/${job.id}`}
                      className="font-medium hover:underline"
                    >
                      {job.name}
                    </a>
                    <Badge variant={config.variant}>
                      <StatusIcon className="mr-1 h-3 w-3" />
                      {config.label}
                    </Badge>
                    {job.host_os && (
                      <Badge variant="outline" className="text-xs">
                        {job.host_os}
                      </Badge>
                    )}
                  </div>
                  <p className="text-muted-foreground text-xs">
                    {job.step_count} step{job.step_count !== 1 ? "s" : ""} &middot;{" "}
                    {job.description.slice(0, 80)}
                    {job.description.length > 80 ? "..." : ""}
                  </p>
                </div>
                <div className="flex items-center gap-2">
                  {(job.status === "draft" || job.status === "queued") && (
                    <Button size="sm" onClick={() => handleStart(job.id)}>
                      <Play className="mr-1 h-3 w-3" />
                      Start
                    </Button>
                  )}
                  {(job.status === "running" || job.status === "queued") && (
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => handleCancel(job.id)}
                    >
                      <XCircle className="mr-1 h-3 w-3" />
                      Cancel
                    </Button>
                  )}
                </div>
              </CardContent>
            </Card>
          );
        })
      )}
    </div>
  );
};
