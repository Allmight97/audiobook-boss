export interface ProcessingStatus {
  stage:
    | "idle"
    | "analyzing"
    | "converting"
    | "writing"
    | "completed"
    | "cancelled"
    | "failed";
  percentage: number;
  message: string;
  currentFile?: string;
  etaSeconds?: number;
}

export type JobStatus = "queued" | "processing" | "completed" | "failed" | "cancelled";

/** Per-job progress tracking for parallel batch processing */
export interface JobProgress {
  jobId?: string;
  inputIndex?: number;
  label: string;
  status: JobStatus;
  stage?: ProcessingStatus["stage"];
  percentage: number;
  message: string;
  lastUpdate: number;
}

/** Aggregate progress across all jobs */
export interface AggregateProgress {
  activeJobs: number;
  queuedJobs: number;
  completedJobs: number;
  overallPercentage: number;
}

export function createInitialStatus(): ProcessingStatus {
  return {
    stage: "idle",
    percentage: 0,
    message: "Ready to process audiobook",
  };
}

/** Calculate aggregate progress across queued, active, and completed jobs */
export function calculateAggregateProgress(
  jobProgress: Map<string, JobProgress>
): AggregateProgress {
  let activeJobs = 0;
  let queuedJobs = 0;
  let completedJobs = 0;
  let totalPercentage = 0;

  for (const job of jobProgress.values()) {
    if (job.status === "queued") {
      queuedJobs++;
      continue;
    }
    if (job.status === "completed") {
      completedJobs++;
      totalPercentage += 100;
      continue;
    }
    if (job.status === "processing") {
      activeJobs++;
      totalPercentage += job.percentage;
      continue;
    }
  }

  const totalJobs = activeJobs + completedJobs;
  // Simple average across active + completed jobs. This keeps the aggregate legible
  // without over-weighting long-running jobs; consider a weighted strategy if we
  // need time/progress proportionality later.
  const overallPercentage = totalJobs > 0 ? totalPercentage / totalJobs : 0;

  return {
    activeJobs,
    queuedJobs,
    completedJobs,
    overallPercentage: Math.round(overallPercentage * 10) / 10,
  };
}

/** Derive aggregate stage from active jobs */
export function deriveAggregateStage(
  jobProgress: Map<string, JobProgress>
): ProcessingStatus["stage"] {
  let hasQueued = false;
  let hasProcessing = false;
  let hasCompleted = false;
  let hasFailed = false;
  let hasCancelled = false;
  const stages: ProcessingStatus["stage"][] = [];

  for (const job of jobProgress.values()) {
    if (job.status === "queued") {
      hasQueued = true;
    } else if (job.status === "completed") {
      hasCompleted = true;
    } else if (job.status === "failed") {
      hasFailed = true;
    } else if (job.status === "cancelled") {
      hasCancelled = true;
    } else if (job.status === "processing") {
      hasProcessing = true;
      if (job.stage) {
        stages.push(job.stage);
      }
    }
  }

  // Priority: failed > cancelled > converting > analyzing > writing > completed > queued > idle
  if (hasFailed) return "failed";
  if (hasCancelled) return "cancelled";
  if (stages.includes("converting")) return "converting";
  if (stages.includes("analyzing")) return "analyzing";
  if (stages.includes("writing")) return "writing";
  if (hasProcessing) return "analyzing";
  if (hasCompleted && !hasQueued) return "completed";
  if (hasQueued) return "analyzing";
  return "idle";
}
