import { formatEtaRemaining } from '../../lib/format/eta';
import { coverSrc } from '../../lib/tauri/coverSrc';
import type { OutputSnapshot, WorkProgressStage } from '../../lib/generated/tauri';
import type { JobListItem, StatusView } from './view';

export type ConcurrencyRead = {
	readonly selection: string;
	readonly effective: number;
};

type PreviewRun = NonNullable<OutputSnapshot['previewRun']>;

function stageText(stage: WorkProgressStage): string {
	switch (stage) {
		case 'pending':
		case 'analyzing':
			return 'Analyzing';
		case 'converting':
			return 'Converting';
		case 'writing':
		case 'cleaning':
			return 'Writing Metadata';
		case 'complete':
			return 'Completed';
		case 'cancelled':
			return 'Cancelled';
		case 'failed':
			return 'Failed';
	}
}

export function concurrencyText(concurrency: ConcurrencyRead): string {
	const suffix = concurrency.selection === 'auto' ? ' (Auto)' : '';
	return `Max jobs: ${concurrency.effective}${suffix}`;
}

function previewStatusText({ status, progress }: PreviewRun['operation']): string {
	if (status === 'mixed') return 'Mixed result';
	if (status === 'cancelling') return 'Cancelling';
	const text = stageText(progress.stage);
	return progress.stage === 'converting' && progress.etaSeconds != null
		? `${text} · ${formatEtaRemaining(progress.etaSeconds)}`
		: text;
}

function previewJobItems({ children, phase }: PreviewRun['operation']): JobListItem[] {
	return children.map((child, index) => ({
		childJobId: child.childJobId,
		label: child.label,
		status: child.status,
		statusText:
			child.cancelRequested && phase !== 'finished'
				? 'Cancelling'
				: child.status === 'queued'
					? `Queued • #${index + 1} of ${children.length}`
					: (child.message ?? child.progress.message),
		stage: child.progress.stage,
		percentage: child.progress.percentage,
		canCancel: child.cancellable,
	}));
}

export function previewView(preview: PreviewRun): Omit<StatusView, 'concurrencyText'> {
	const { operation } = preview;
	return {
		coverArtSrc: preview.cover ? coverSrc(preview.cover) : null,
		jobItems: previewJobItems(operation),
		progressPercentage: operation.progress.percentage,
		statusText: previewStatusText(operation),
		stepText: `Current Step: ${operation.progress.message}`,
		stepTone: 'normal',
		progressActive: operation.phase === 'active',
	};
}
