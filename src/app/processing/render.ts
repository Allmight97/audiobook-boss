import { formatEtaRemaining } from '../../lib/format/eta';
import type { StatusViewStore } from './view';
import type { SessionOutput } from '../../types/session';
import type { WorkProgressStage } from '../../types/workRuntime';
import { isWorkRunning } from '../workOperations';

export type ConcurrencyRead = {
	readonly selection: string;
	readonly effective: number | null;
};

export type StatusLine = {
	readonly stage: WorkProgressStage;
	readonly percentage: number;
	readonly message: string;
	readonly etaSeconds?: number | null;
};

function stageText(stage: WorkProgressStage): string {
	switch (stage) {
		case 'pending':
		case 'analyzing':
			return 'Analyzing';
		case 'converting':
			return 'Converting';
		case 'writing':
		case 'committing':
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

export function renderStatus(view: StatusViewStore, status: StatusLine): void {
	view.setProgressPercentage(status.percentage);
	const text = stageText(status.stage);
	view.setStatusText(
		status.stage === 'converting' && status.etaSeconds != null
			? `${text} · ${formatEtaRemaining(status.etaSeconds)}`
			: text,
	);
	view.setStepText(`Current Step: ${status.message}`);
	view.setStepColor('var(--text-primary)');
	view.setProgressActive(false);
}

export function renderConcurrencyStatus(
	view: StatusViewStore,
	concurrency: ConcurrencyRead | undefined,
): void {
	const effective = concurrency?.effective ?? null;
	const suffix = concurrency?.selection === 'auto' ? ' (Auto)' : '';

	if (effective === null) {
		view.setConcurrencyText('Max jobs: —');
		return;
	}

	view.setConcurrencyText(`Max jobs: ${effective}${suffix}`);
}

export function renderPreview(view: StatusViewStore, preview: SessionOutput['previewRun']): void {
	if (!preview) return;
	const { operation } = preview;
	const active =
		operation.status === 'accepted' ||
		operation.status === 'running' ||
		operation.status === 'cancelling';
	renderStatus(view, operation.progress);
	view.setProgressActive(isWorkRunning(operation.status));
	if (operation.status === 'mixed') view.setStatusText('Mixed result');
	if (operation.status === 'cancelling') view.setStatusText('Cancelling');
	view.setJobItems(
		operation.children.map((child, index) => ({
			childJobId: child.childJobId,
			label: child.label,
			status: child.status,
			statusText:
				child.cancelRequested && active
					? 'Cancelling'
					: child.status === 'queued'
						? `Queued • #${index + 1} of ${operation.children.length}`
						: (child.message ?? child.progress.message),
			stage: child.progress.stage,
			percentage: child.progress.percentage,
			canCancel: child.cancellable,
		})),
	);
}
