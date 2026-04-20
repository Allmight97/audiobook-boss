import type { EventStage } from '../../types/events';

/**
 * Stage values emitted from Rust that represent ongoing work. The companion
 * `ACTIVE_EVENT_STAGES` map below is typed against this alias, so a new active
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

const ACTIVE_EVENT_STAGES: { readonly [K in ActiveEventStage]: true } = {
	analyzing: true,
	converting: true,
	writing: true,
};

const ACTIVE_STAGE_PRIORITY: { readonly [K in ActiveEventStage]: number } = {
	converting: 0,
	analyzing: 1,
	writing: 2,
};

export function isActiveEventStage(stage: ProcessingStatus['stage']): stage is ActiveEventStage {
	return stage !== 'idle' && stage in ACTIVE_EVENT_STAGES;
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

/** Calculate aggregate progress across queued, active, and completed jobs */
export function calculateAggregateProgress(
	jobProgress: Map<string, JobProgress>,
): AggregateProgress {
	return calculateAggregateProgressAndStage(jobProgress).aggregate;
}

export function calculateAggregateProgressAndStage(
	jobProgress: Map<string, JobProgress>,
): AggregateProgressAndStage {
	let activeJobs = 0;
	let queuedJobs = 0;
	let completedJobs = 0;
	let totalPercentage = 0;
	let hasQueued = false;
	let hasProcessing = false;
	let hasCompleted = false;
	let hasFailed = false;
	let hasCancelled = false;
	let highestPriorityActiveStage: ActiveEventStage | null = null;

	for (const job of jobProgress.values()) {
		if (job.status === 'queued') {
			queuedJobs++;
			hasQueued = true;
			continue;
		}
		if (job.status === 'completed') {
			completedJobs++;
			hasCompleted = true;
			totalPercentage += 100;
			continue;
		}
		if (job.status === 'skipped') {
			completedJobs++;
			hasCompleted = true;
			totalPercentage += 100;
			continue;
		}
		if (job.status === 'failed') {
			hasFailed = true;
			continue;
		}
		if (job.status === 'cancelled') {
			hasCancelled = true;
			continue;
		}
		if (job.status === 'processing') {
			activeJobs++;
			hasProcessing = true;
			totalPercentage += job.percentage;
			if (job.stage && isActiveEventStage(job.stage)) {
				if (
					highestPriorityActiveStage === null ||
					ACTIVE_STAGE_PRIORITY[job.stage] < ACTIVE_STAGE_PRIORITY[highestPriorityActiveStage]
				) {
					highestPriorityActiveStage = job.stage;
				}
			}
		}
	}

	const totalJobs = activeJobs + completedJobs + queuedJobs;
	// Aggregate progress treats queued jobs as 0%-complete participants so mixed
	// completed/queued batches do not misleadingly report 100% before queued work starts.
	// This remains a simple per-job average rather than a duration-weighted model.
	const overallPercentage = totalJobs > 0 ? totalPercentage / totalJobs : 0;

	const aggregate = {
		activeJobs,
		queuedJobs,
		completedJobs,
		overallPercentage: Math.round(overallPercentage * 10) / 10,
	};

	// Priority: failed > cancelled > converting > analyzing > writing > completed > queued > idle
	let stage: ProcessingStatus['stage'];
	if (hasFailed) stage = 'failed';
	else if (hasCancelled) stage = 'cancelled';
	else if (highestPriorityActiveStage) stage = highestPriorityActiveStage;
	else if (hasProcessing) stage = 'analyzing';
	else if (hasCompleted && !hasQueued) stage = 'completed';
	else if (hasQueued) stage = 'analyzing';
	else stage = 'idle';

	return { aggregate, stage };
}

/** Derive aggregate stage from active jobs */
export function deriveAggregateStage(
	jobProgress: Map<string, JobProgress>,
): ProcessingStatus['stage'] {
	return calculateAggregateProgressAndStage(jobProgress).stage;
}
