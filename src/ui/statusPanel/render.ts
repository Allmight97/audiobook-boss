import { formatStatusDisplayText } from './formatting';
import type { JobProgress, ProcessingStatus } from './state';
import {
	setStatusPanelEtaSeconds,
	setStatusPanelForegroundJobLabel,
	setStatusPanelHasCancellableForegroundJob,
	setStatusPanelIsProcessing,
	setStatusPanelProgressPercentage,
	setStatusPanelStatusText,
	setStatusPanelStepColor,
	setStatusPanelStepText,
} from './viewState.svelte';

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

export function renderTransportSummary(
	jobProgress: Map<string, JobProgress>,
	queueOrder: string[],
): void {
	const orderedKeys = buildRenderOrder(jobProgress, queueOrder);
	const orderedJobs = orderedKeys
		.map((key) => jobProgress.get(key))
		.filter((job): job is JobProgress => job !== undefined);
	const foregroundJob = orderedJobs.find((job) => job.status === 'processing') ?? orderedJobs[0];

	setStatusPanelForegroundJobLabel(foregroundJob?.label ?? null);
	setStatusPanelHasCancellableForegroundJob(
		orderedJobs.some((job) => job.status === 'processing' && Boolean(job.jobId)),
	);
}
