import { getMaxConcurrentStatus } from "../jobControls";
import * as dom from "./dom";
import { formatStatusDisplayText } from "./formatting";
import type { AggregateProgress, JobProgress, ProcessingStatus } from "./state";

export function renderStatus(
  status: ProcessingStatus,
  isProcessing: boolean
): void {
  dom.updateProgressBar(status.percentage);
  dom.updatePercentageText(status.percentage);
  dom.updateStatusText(formatStatusDisplayText(status.stage));
  dom.updateStepText(`Current Step: ${status.message}`);
  dom.updateProcessButton(isProcessing);
}

export function renderConcurrencyStatus(aggregate?: AggregateProgress): void {
  const { effective, selection } = getMaxConcurrentStatus();
  const suffix = selection === "auto" ? " (Auto)" : "";

  if (effective === null) {
    dom.updateConcurrencyStatus("Max jobs: —");
    return;
  }

  if (aggregate && (aggregate.activeJobs > 0 || aggregate.completedJobs > 0)) {
    const completedSuffix =
      aggregate.completedJobs > 0
        ? ` • Completed ${aggregate.completedJobs}`
        : "";
    dom.updateConcurrencyStatus(
      `Running ${aggregate.activeJobs} / Max ${effective}${suffix}${completedSuffix}`
    );
    return;
  }

  dom.updateConcurrencyStatus(`Max jobs: ${effective}${suffix}`);
}

export function renderJobList(
  jobProgress: Map<string, JobProgress>,
  onCancel: (id: string) => void
): void {
  const jobs = Array.from(jobProgress.values())
    .sort((a, b) => b.lastUpdate - a.lastUpdate)
    .map((job) => ({
      id: job.jobId,
      label: `${job.jobId.slice(0, 8)} • ${job.message}`,
      stage: job.stage,
      percentage: job.percentage,
      onCancel: job.jobId === "default" ? undefined : onCancel,
    }));

  dom.renderJobList(jobs);
}
