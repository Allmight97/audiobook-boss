import { formatEtaRemaining } from '../../lib/format/eta';
import { coverSrc } from '../../lib/tauri/coverSrc';
import type { OutputSnapshot } from '../../lib/generated/tauri';
import { childCancelling, operationStatusLabel, stageStatusText } from '../workOperations';
import type { JobListItem, StatusView } from './view';

type PreviewRun = NonNullable<OutputSnapshot['previewRun']>;

function previewStatusText({ status, progress }: PreviewRun['operation']): string {
	if (status === 'mixed' || status === 'cancelling') return operationStatusLabel(status);
	const text = stageStatusText(progress.stage);
	return progress.stage === 'converting' && progress.etaSeconds != null
		? `${text} · ${formatEtaRemaining(progress.etaSeconds)}`
		: text;
}

function previewJobItems({ children }: PreviewRun['operation']): JobListItem[] {
	return children.map((child, index) => ({
		childJobId: child.childJobId,
		label: child.label,
		status: child.status,
		statusText: childCancelling(child)
			? operationStatusLabel('cancelling')
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
