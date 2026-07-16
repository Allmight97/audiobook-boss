import { formatStatusDisplayText } from './formatting';
import type { JobProgress, ProcessingStatus } from './state';
import {
	setStatusPanelEtaSeconds,
	setStatusPanelIsProcessing,
	setStatusPanelJobItems,
	setStatusPanelProgressPercentage,
	setStatusPanelStatusText,
	setStatusPanelStepColor,
	setStatusPanelStepText,
} from './viewState.svelte';
import type { JobListItem } from './viewTypes';

export function renderStatus(status: ProcessingStatus, isProcessing: boolean): void {
	setStatusPanelProgressPercentage(status.percentage);
	setStatusPanelStatusText(formatStatusDisplayText(status.stage));
	setStatusPanelStepText(`Current Step: ${status.message}`);
	setStatusPanelStepColor('var(--text-primary)');
	setStatusPanelIsProcessing(isProcessing);
	setStatusPanelEtaSeconds(
		status.stage === 'converting' && typeof status.etaSeconds === 'number'
			? status.etaSeconds
			: null,
	);
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

	setStatusPanelJobItems(jobs);
}
