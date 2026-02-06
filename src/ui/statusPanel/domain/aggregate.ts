import {
  calculateAggregateProgress as calculateAggregateProgressState,
  calculateAggregateProgressAndStage as calculateAggregateProgressAndStageState,
  deriveAggregateStage as deriveAggregateStageState,
  type AggregateProgressAndStage,
  type AggregateProgress,
  type JobProgress,
  type ProcessingStatus,
} from "../state";

export type { AggregateProgress };

export function calculateAggregateProgress(
  jobProgress: Map<string, JobProgress>
): AggregateProgress {
  return calculateAggregateProgressState(jobProgress);
}

export function deriveAggregateStage(
  jobProgress: Map<string, JobProgress>
): ProcessingStatus["stage"] {
  return deriveAggregateStageState(jobProgress);
}

export function calculateAggregateProgressAndStage(
  jobProgress: Map<string, JobProgress>
): AggregateProgressAndStage {
  return calculateAggregateProgressAndStageState(jobProgress);
}
