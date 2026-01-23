/**
 * Pure functions for job state transformations
 *
 * This module contains pure functions for managing job progress state
 * during batch processing. Extracted from logic.ts to improve separation
 * of concerns and testability.
 */

import { STAGES } from "../../types/events";
import type {
  ProcessingProgressEvent,
  ProcessingQueueEvent,
} from "../../types/events";
import { buildQueueLabels } from "./formatting";
import type { JobProgress, JobStatus } from "./state";

// === Key Building ===

/**
 * Build a unique key for a job based on input index or job ID
 */
export function buildJobKey(inputIndex?: number, jobId?: string): string {
  if (typeof inputIndex === "number") {
    return `idx:${inputIndex}`;
  }
  if (jobId) {
    return `job:${jobId}`;
  }
  return "default";
}

// === Queue Snapshot Processing ===

export interface QueueSnapshotResult {
  jobs: Map<string, JobProgress>;
  order: string[];
}

/**
 * Process a queue snapshot event into job state
 */
export function processQueueSnapshot(
  event: ProcessingQueueEvent,
  now: number = Date.now()
): QueueSnapshotResult {
  const filePaths = event.items.map((item) => item.file_path);
  const labels = buildQueueLabels(filePaths);

  const jobs = new Map<string, JobProgress>();
  const order: string[] = [];

  event.items.forEach((item, index) => {
    const key = buildJobKey(item.input_index, undefined);
    order.push(key);
    jobs.set(key, {
      inputIndex: item.input_index,
      label: labels[index] ?? item.file_path,
      status: "queued",
      percentage: 0,
      message: "Queued",
      lastUpdate: now,
    });
  });

  return { jobs, order };
}

// === Progress Update Processing ===

export interface ProgressUpdateResult {
  job: JobProgress;
  isTerminal: boolean;
}

/**
 * Process a progress update event into job state
 */
export function processProgressUpdate(
  event: ProcessingProgressEvent,
  existing: JobProgress | undefined,
  fallbackLabel: string,
  now: number = Date.now()
): ProgressUpdateResult {
  const isTerminal =
    event.stage === STAGES.completed ||
    event.stage === STAGES.failed ||
    event.stage === STAGES.cancelled;

  const jobStatus: JobStatus = isTerminal
    ? (event.stage as JobStatus)
    : "processing";

  const job: JobProgress = {
    jobId: event.job_id ?? existing?.jobId,
    inputIndex:
      typeof event.input_index === "number"
        ? event.input_index
        : existing?.inputIndex,
    label: existing?.label ?? fallbackLabel,
    status: jobStatus,
    stage: event.stage,
    percentage: Math.round(event.percentage * 10) / 10,
    message: event.message,
    lastUpdate: now,
  };

  return { job, isTerminal };
}

// === Throttle Check ===

/**
 * Check if a progress update should be throttled
 */
export function shouldThrottleUpdate(
  jobKey: string,
  isTerminal: boolean,
  lastRenderTimes: Map<string, number>,
  now: number,
  throttleMs: number = 500
): boolean {
  if (isTerminal) {
    return false; // Never throttle terminal events
  }

  const lastRender = lastRenderTimes.get(jobKey) ?? 0;
  return now - lastRender < throttleMs;
}

// === Terminal State Checks ===

/**
 * Check if all jobs in the queue are in a terminal state
 */
export function areAllJobsTerminal(
  jobs: Map<string, JobProgress>,
  order: string[]
): boolean {
  if (order.length === 0) return false;

  return order.every((key) => {
    const job = jobs.get(key);
    return (
      job &&
      (job.status === "completed" ||
        job.status === "failed" ||
        job.status === "cancelled")
    );
  });
}

/**
 * Check if any job has failed
 */
export function hasFailedJobs(jobs: Map<string, JobProgress>): boolean {
  return Array.from(jobs.values()).some((job) => job.status === "failed");
}

/**
 * Check if any job has been cancelled
 */
export function hasCancelledJobs(jobs: Map<string, JobProgress>): boolean {
  return Array.from(jobs.values()).some((job) => job.status === "cancelled");
}
