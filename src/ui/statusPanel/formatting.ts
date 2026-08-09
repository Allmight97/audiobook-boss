import { pathSegments } from '../../lib/path/basename';
import type { AggregateProgress, JobProgress, ProcessingStatus } from './state';

export { formatEtaRemaining } from '../../lib/format/eta';

function assertNever(value: never): never {
	throw new Error(`Unhandled status stage: ${String(value)}`);
}

export function formatStatusDisplayText(stage: ProcessingStatus['stage']): string {
	switch (stage) {
		case 'idle':
			return 'Idle';
		case 'analyzing':
			return 'Analyzing';
		case 'converting':
			return 'Converting';
		case 'writing':
			return 'Writing Metadata';
		case 'completed':
			return 'Completed';
		case 'skipped':
			return 'Skipped';
		case 'cancelled':
			return 'Cancelled';
		case 'failed':
			return 'Failed';
	}
	return assertNever(stage);
}

export function formatAggregateMessage(
	jobProgress: Map<string, JobProgress>,
	aggregate: AggregateProgress,
): string {
	if (aggregate.activeJobs > 1) {
		const queuedSuffix = aggregate.queuedJobs > 0 ? `, ${aggregate.queuedJobs} queued` : '';
		const completedSuffix =
			aggregate.completedJobs > 0 ? `, ${aggregate.completedJobs} completed` : '';
		return `Processing ${aggregate.activeJobs} files${queuedSuffix}${completedSuffix}`;
	}
	if (aggregate.activeJobs === 1) {
		const activeJob = Array.from(jobProgress.values()).find((job) => job.status === 'processing');
		return activeJob?.message ?? 'Processing...';
	}
	if (aggregate.queuedJobs > 0) {
		return `Queued ${aggregate.queuedJobs} file${aggregate.queuedJobs === 1 ? '' : 's'}`;
	}
	return 'Ready to process audiobook';
}

export function extractFilenameFromProgress(label: string): string | null {
	const trimmed = label.trim();
	if (!trimmed) return null;
	const match = trimmed.match(/^(.*?) \(\d+\/\d+\)$/);
	if (match?.[1]) {
		return match[1].trim();
	}
	return trimmed;
}

export function buildQueueLabels(paths: string[]): string[] {
	const segmentsList = paths.map(pathSegments);
	const maxDepth = segmentsList.reduce((max, segments) => Math.max(max, segments.length), 0);

	for (let depth = 1; depth <= maxDepth; depth += 1) {
		const labels = segmentsList.map((segments) =>
			segments.length > 0 ? segments.slice(-depth).join('/') : '',
		);
		const counts = labels.reduce<Record<string, number>>((acc, label) => {
			acc[label] = (acc[label] ?? 0) + 1;
			return acc;
		}, {});
		const hasDuplicates = labels.some((label) => label !== '' && (counts[label] ?? 0) > 1);
		if (!hasDuplicates) return labels;
	}

	const labels = segmentsList.map((segments) => segments.join('/'));
	const counts = labels.reduce<Record<string, number>>((acc, label) => {
		acc[label] = (acc[label] ?? 0) + 1;
		return acc;
	}, {});
	return labels.map((label, index) => {
		if ((counts[label] ?? 0) <= 1) return label;
		return `${label} (${index + 1})`;
	});
}
