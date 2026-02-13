export interface ProcessingStatus {
	stage: 'idle' | 'analyzing' | 'converting' | 'writing' | 'completed' | 'cancelled' | 'failed';
	percentage: number;
	message: string;
	currentFile?: string;
	etaSeconds?: number;
}

export type JobStatus = 'queued' | 'processing' | 'completed' | 'failed' | 'cancelled';

/** Per-job progress tracking for parallel batch processing */
export interface JobProgress {
	jobId?: string;
	inputIndex?: number;
	label: string;
	status: JobStatus;
	stage?: ProcessingStatus['stage'];
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
	let hasConverting = false;
	let hasAnalyzing = false;
	let hasWriting = false;

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
			if (job.stage === 'converting') {
				hasConverting = true;
			} else if (job.stage === 'analyzing') {
				hasAnalyzing = true;
			} else if (job.stage === 'writing') {
				hasWriting = true;
			}
		}
	}

	const totalJobs = activeJobs + completedJobs;
	// Simple average across active + completed jobs. This keeps the aggregate legible
	// without over-weighting long-running jobs; consider a weighted strategy if we
	// need time/progress proportionality later.
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
	else if (hasConverting) stage = 'converting';
	else if (hasAnalyzing) stage = 'analyzing';
	else if (hasWriting) stage = 'writing';
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
