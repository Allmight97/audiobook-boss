import { getMaxConcurrentStatus } from '../jobControls';
import * as feedback from './feedback';
import { formatStatusDisplayText } from './formatting';
import type { AggregateProgress, JobProgress, ProcessingStatus } from './state';
import type { JobListItem } from './viewTypes';

export function renderStatus(status: ProcessingStatus, isProcessing: boolean): void {
	feedback.updateProgressBar(status.percentage);
	feedback.updatePercentageText(status.percentage);
	feedback.updateStatusText(formatStatusDisplayText(status.stage));
	feedback.updateStepText(`Current Step: ${status.message}`);
	feedback.updateProcessButton(isProcessing);
}

export function renderConcurrencyStatus(aggregate?: AggregateProgress): void {
	const { effective, selection } = getMaxConcurrentStatus();
	const suffix = selection === 'auto' ? ' (Auto)' : '';

	if (effective === null) {
		feedback.updateConcurrencyStatus('Max jobs: —');
		return;
	}

	if (
		aggregate &&
		(aggregate.activeJobs > 0 || aggregate.completedJobs > 0 || aggregate.queuedJobs > 0)
	) {
		const queuedSuffix = aggregate.queuedJobs > 0 ? ` • Queued ${aggregate.queuedJobs}` : '';
		const completedSuffix =
			aggregate.completedJobs > 0 ? ` • Completed ${aggregate.completedJobs}` : '';
		feedback.updateConcurrencyStatus(
			`Running ${aggregate.activeJobs} / Max ${effective}${suffix}${queuedSuffix}${completedSuffix}`,
		);
		return;
	}

	feedback.updateConcurrencyStatus(`Max jobs: ${effective}${suffix}`);
}

function formatJobStatusText(
	job: JobProgress,
	position: number | null,
	total: number | null,
): string {
	if (typeof job.inputIndex !== 'number') {
		return job.message;
	}
	if (job.status === 'queued') {
		if (position && total) {
			return `Queued • #${position} of ${total}`;
		}
		return 'Queued';
	}
	if (job.status === 'processing') {
		const stage = job.stage ?? 'analyzing';
		return formatStatusDisplayText(stage);
	}
	if (job.status === 'completed') {
		return 'Completed';
	}
	if (job.status === 'skipped') {
		return 'Skipped';
	}
	if (job.status === 'failed') {
		return 'Failed';
	}
	if (job.status === 'cancelled') {
		return 'Cancelled';
	}
	return 'Processing';
}

function buildRenderOrder(jobProgress: Map<string, JobProgress>, queueOrder: string[]): string[] {
	if (queueOrder.length === 0) {
		return Array.from(jobProgress.entries())
			.sort((a, b) => b[1].lastUpdate - a[1].lastUpdate)
			.map(([key]) => key);
	}

	const seen = new Set(queueOrder);
	const extras = Array.from(jobProgress.entries())
		.filter(([key]) => !seen.has(key))
		.sort((a, b) => b[1].lastUpdate - a[1].lastUpdate)
		.map(([key]) => key);

	return [...queueOrder, ...extras];
}

export function renderJobList(
	jobProgress: Map<string, JobProgress>,
	queueOrder: string[],
	onCancel: (id: string) => void,
): void {
	const orderedKeys = buildRenderOrder(jobProgress, queueOrder);
	const total = queueOrder.length > 0 ? queueOrder.length : null;
	const queuePositions =
		queueOrder.length > 0
			? queueOrder.reduce<Map<string, number>>((positions, key, index) => {
					positions.set(key, index + 1);
					return positions;
				}, new Map<string, number>())
			: null;

	const jobs: JobListItem[] = orderedKeys.reduce<JobListItem[]>((acc, key) => {
		const job = jobProgress.get(key);
		if (!job) return acc;
		const position = queuePositions?.get(key) ?? null;
		const statusText = formatJobStatusText(job, position, total);
		const canCancel = job.status === 'processing' && !!job.jobId;
		const percentage =
			job.status === 'processing' || job.status === 'completed' || job.status === 'skipped'
				? job.percentage
				: undefined;

		const item: JobListItem = {
			key,
			label: job.label,
			status: job.status,
			statusText,
			stage: job.stage,
			canCancel,
			cancelId: job.jobId,
			onCancel: canCancel ? onCancel : undefined,
		};

		if (typeof percentage === 'number') {
			item.percentage = percentage;
		}

		acc.push(item);
		return acc;
	}, []);

	feedback.renderJobList(jobs);
}
