import {
  calculateAggregateProgress as calculateAggregateProgressState,
  deriveAggregateStage as deriveAggregateStageState,
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
