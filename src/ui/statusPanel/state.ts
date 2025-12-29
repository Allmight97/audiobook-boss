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

/** Per-job progress tracking for parallel batch processing */
export interface JobProgress {
  jobId: string;
  stage: ProcessingStatus["stage"];
  percentage: number;
  message: string;
  lastUpdate: number;
}

/** Aggregate progress across all active jobs */
export interface AggregateProgress {
  activeJobs: number;
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

/** Calculate aggregate progress across all active jobs */
export function calculateAggregateProgress(
  jobProgress: Map<string, JobProgress>
): AggregateProgress {
  let activeJobs = 0;
  let completedJobs = 0;
  let totalPercentage = 0;

  for (const job of jobProgress.values()) {
    if (job.stage === "completed") {
      completedJobs++;
      totalPercentage += 100;
    } else if (job.stage === "failed" || job.stage === "cancelled") {
      // Don't count failed/cancelled in active or completed
    } else {
      activeJobs++;
      totalPercentage += job.percentage;
    }
  }

  const totalJobs = activeJobs + completedJobs;
  // Simple average across active + completed jobs. This keeps the aggregate legible
  // without over-weighting long-running jobs; consider a weighted strategy if we
  // need time/progress proportionality later.
  const overallPercentage = totalJobs > 0 ? totalPercentage / totalJobs : 0;

  return {
    activeJobs,
    completedJobs,
    overallPercentage: Math.round(overallPercentage * 10) / 10,
  };
}

/** Derive aggregate stage from active jobs */
export function deriveAggregateStage(
  jobProgress: Map<string, JobProgress>
): ProcessingStatus["stage"] {
  const stages = Array.from(jobProgress.values()).map((job) => job.stage);

  // Priority: failed > cancelled > converting > analyzing > writing > completed > idle
  if (stages.includes("failed")) return "failed";
  if (stages.includes("cancelled")) return "cancelled";
  if (stages.includes("converting")) return "converting";
  if (stages.includes("analyzing")) return "analyzing";
  if (stages.includes("writing")) return "writing";
  if (stages.includes("completed")) return "completed";
  return "idle";
}
