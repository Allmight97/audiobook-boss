import type { ProcessingQueueItem } from "../../../types/events";
import { buildQueueLabels } from "../formatting";
import type { JobProgress } from "../state";
import { buildJobKey } from "./jobKeys";

export interface QueueSnapshotState {
  queueOrder: string[];
  jobProgress: Map<string, JobProgress>;
}

export function buildQueueSnapshotState(
  items: ProcessingQueueItem[],
  now: number
): QueueSnapshotState {
  const labels = buildQueueLabels(items.map((item) => item.file_path));
  const queueOrder: string[] = [];
  const jobProgress = new Map<string, JobProgress>();

  items.forEach((item, index) => {
    const key = buildJobKey(item.input_index, undefined);
    queueOrder.push(key);
    jobProgress.set(key, {
      inputIndex: item.input_index,
      label: labels[index] ?? item.file_path,
      status: "queued",
      percentage: 0,
      message: "Queued",
      lastUpdate: now,
    });
  });

  return { queueOrder, jobProgress };
}

export function areAllBatchJobsTerminal(
  queueOrder: string[],
  jobProgress: Map<string, JobProgress>
): boolean {
  if (queueOrder.length === 0) return false;

  return queueOrder.every((key) => {
    const job = jobProgress.get(key);
    return (
      job &&
      (job.status === "completed" ||
        job.status === "failed" ||
        job.status === "cancelled")
    );
  });
}
