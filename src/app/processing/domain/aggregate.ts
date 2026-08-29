import {
	isActiveEventStage,
	type ActiveEventStage,
	type AggregateProgressAndStage,
	type JobProgress,
	type ProcessingStatus,
} from '../state';

const ACTIVE_STAGE_PRIORITY: { readonly [K in ActiveEventStage]: number } = {
	converting: 0,
	analyzing: 1,
	writing: 2,
};

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
		if (job.status === 'completed' || job.status === 'skipped') {
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
