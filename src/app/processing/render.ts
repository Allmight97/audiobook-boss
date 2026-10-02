import { formatEtaRemaining, formatStatusDisplayText } from './formatting';
import type { ProcessingStatus } from './state';
import type { StatusViewStore } from './view';
import type { SessionOutput } from '../../types/session';
import type { WorkProgressStage } from '../../types/workRuntime';
import { buildStatus } from './state';

export type ConcurrencyRead = {
	readonly selection: string;
	readonly effective: number | null;
};

export function renderStatus(
	view: StatusViewStore,
	status: ProcessingStatus,
	isProcessing: boolean,
): void {
	view.setProgressPercentage(status.percentage);
	const statusText =
		status.stage === 'converting' && status.etaSeconds !== undefined
			? `${formatStatusDisplayText(status.stage)} · ${formatEtaRemaining(status.etaSeconds)}`
			: formatStatusDisplayText(status.stage);
	view.setStatusText(statusText);
	view.setStepText(`Current Step: ${status.message}`);
	view.setStepColor('var(--text-primary)');
	view.setIsProcessing(isProcessing);
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

export function renderPreview(
	view: StatusViewStore,
	preview: SessionOutput['previewRun'],
	cancel: (child: string) => void,
): void {
	if (!preview) return;
	const { operation } = preview;
	const active =
		operation.status === 'accepted' ||
		operation.status === 'running' ||
		operation.status === 'cancelling';
	const stage = stageForProgress(operation.progress.stage);
	const message = operation.terminalSummary?.message ?? operation.progress.message;
	renderStatus(
		view,
		buildStatus(stage, operation.progress.percentage, message, {
			etaSeconds: operation.progress.etaSeconds,
		}),
		active,
	);
	if (operation.status === 'mixed') view.setStatusText('Mixed result');
	if (operation.status === 'cancelling') view.setStatusText('Cancelling');
	view.setCancelAllPending(operation.cancelRequested && active);
	view.setJobItems(
		operation.children.map((child, index) => ({
			key: `${operation.operationId}:${child.childJobId}`,
			label: child.label,
			status: child.status === 'running' ? 'processing' : child.status,
			statusText:
				child.cancelRequested && active
					? 'Cancelling'
					: child.status === 'queued'
						? `Queued • #${index + 1} of ${operation.children.length}`
						: (child.message ?? child.progress.message),
			stage: stageForProgress(child.progress.stage),
			percentage: child.progress.percentage,
			canCancel: child.cancellable && active,
			cancelId: child.childJobId,
			onCancel: cancel,
		})),
	);
}

function stageForProgress(
	stage: WorkProgressStage,
): Exclude<ProcessingStatus['stage'], 'idle' | 'skipped'> {
	switch (stage) {
		case 'complete':
			return 'completed';
		case 'failed':
			return 'failed';
		case 'cancelled':
			return 'cancelled';
		case 'converting':
			return 'converting';
		case 'writing':
		case 'committing':
		case 'cleaning':
			return 'writing';
		default:
			return 'analyzing';
	}
}
