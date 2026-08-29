import type { EventStage } from '../../types/events';

/**
 * Stage values emitted from Rust that represent ongoing work. The companion
 * `isActiveEventStage` guard below is typed against this alias, so a new active
 * Rust stage must be acknowledged here before the frontend compiles again.
 */
export type ActiveEventStage = Exclude<
	EventStage,
	'completed' | 'skipped' | 'failed' | 'cancelled'
>;

/**
 * UI-layer processing status modeled as a discriminated union so that fields
 * only meaningful during active work (`currentFile`, `etaSeconds`) are
 * unrepresentable on terminal and idle variants.
 *
 * `idle` is a UI-only stage — there is no corresponding wire value. Active
 * variants track `ActiveEventStage`, which itself derives from the Rust-authored
 * `EventStage` (see `src/types/events.ts`), so the flow `EventStage -> wire ->
 * UI` stays consistent at the type level. Idle is pinned to `percentage: 0`,
 * making the old "idle with stale progress" state unrepresentable.
 */
export type ProcessingStatus =
	| { stage: 'idle'; percentage: 0; message: string }
	| {
			stage: ActiveEventStage;
			percentage: number;
			message: string;
			currentFile?: string;
			etaSeconds?: number;
	  }
	| { stage: 'completed'; percentage: number; message: string }
	| { stage: 'skipped'; percentage: number; message: string }
	| { stage: 'failed'; percentage: number; message: string }
	| { stage: 'cancelled'; percentage: number; message: string };

export type JobStatus = 'queued' | 'processing' | 'completed' | 'skipped' | 'failed' | 'cancelled';

/**
 * Per-job progress tracking for queued and running work.
 *
 * `stage` mirrors backend-emitted `EventStage` values only. The UI-only `idle`
 * variant from `ProcessingStatus` must never appear on an individual job row.
 */
export interface JobProgress {
	jobId?: string;
	inputIndex?: number;
	label: string;
	status: JobStatus;
	stage?: EventStage;
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

export interface AggregateProgressAndStage {
	aggregate: AggregateProgress;
	stage: ProcessingStatus['stage'];
}

export function createInitialStatus(): ProcessingStatus {
	return {
		stage: 'idle',
		percentage: 0,
		message: 'Ready to process audiobook',
	};
}

export function isActiveEventStage(stage: ProcessingStatus['stage']): stage is ActiveEventStage {
	return stage === 'analyzing' || stage === 'converting' || stage === 'writing';
}

/**
 * Factory that constructs the correct `ProcessingStatus` variant based on the
 * `stage` discriminant. `active` extras are applied only when `stage` is an
 * active event stage; for terminal and idle stages the extras are ignored at
 * the type level. Idle is always normalized to `percentage: 0`, even if an
 * out-of-band caller passes a stale numeric value.
 */
export function buildStatus(
	stage: ProcessingStatus['stage'],
	percentage: number,
	message: string,
	active?: { currentFile?: string | null; etaSeconds?: number | null },
): ProcessingStatus {
	if (stage === 'idle') {
		return { stage, percentage: 0, message };
	}

	if (isActiveEventStage(stage)) {
		const currentFile = active?.currentFile ?? undefined;
		const etaSeconds = active?.etaSeconds ?? undefined;
		return {
			stage,
			percentage,
			message,
			...(currentFile !== undefined ? { currentFile } : {}),
			...(etaSeconds !== undefined ? { etaSeconds } : {}),
		};
	}
	return { stage, percentage, message };
}
