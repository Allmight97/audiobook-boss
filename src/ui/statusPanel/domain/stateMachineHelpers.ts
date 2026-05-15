import { STAGES, type EventStage, type ProcessingProgressEvent } from '../../../types/events';
import type { ProcessCommandResult } from '../../../types/audio';
import { extractFilenameFromProgress, formatAggregateMessage } from '../formatting';
import { buildStatus, type JobProgress, type JobStatus } from '../state';
import { calculateAggregateProgressAndStage } from './aggregate';
import { buildJobKey } from './jobKeys';
import type {
	SingleCompletionHoldIntent,
	StatusPanelCompletionFeedback,
	StatusPanelModel,
} from './stateMachineTypes';

export function buildBatchCompletionFeedback(
	model: StatusPanelModel,
): StatusPanelCompletionFeedback {
	const statuses = Array.from(model.jobProgress.values()).map((job) => job.status);
	const hasFailed = statuses.includes('failed');
	const hasCancelled = statuses.includes('cancelled');
	const hasCompleted = statuses.includes('completed');
	const hasSkipped = statuses.includes('skipped');

	if (model.batchCompletionMessageOverride !== null) {
		return {
			kind: hasFailed ? 'error' : hasCancelled ? 'info' : hasCompleted ? 'success' : 'info',
			message: model.batchCompletionMessageOverride,
		};
	}

	if (hasFailed) {
		return {
			kind: 'error',
			message: 'One or more files failed to process.',
		};
	}

	if (hasCancelled) {
		return {
			kind: 'info',
			message: 'Processing was cancelled.',
		};
	}

	if (!hasCompleted && hasSkipped) {
		return {
			kind: 'info',
			message: 'No files were processed.',
		};
	}

	return {
		kind: 'success',
		message: 'Audiobook created successfully!',
	};
}

export function buildSingleCompletionFeedback(
	event: Pick<SingleCompletionHoldIntent, 'terminalStage' | 'message'>,
): StatusPanelCompletionFeedback {
	if (event.terminalStage === STAGES.completed) {
		return { kind: 'success', message: 'Audiobook created successfully!' };
	}
	if (event.terminalStage === STAGES.failed) {
		return { kind: 'error', message: event.message };
	}
	if (event.terminalStage === STAGES.skipped) {
		return { kind: 'info', message: event.message || 'No files were processed.' };
	}
	return { kind: 'info', message: 'Processing was cancelled.' };
}

export function recomputeStatus(model: StatusPanelModel): StatusPanelModel {
	const { aggregate, stage } = calculateAggregateProgressAndStage(model.jobProgress);
	const status = buildStatus(
		stage,
		aggregate.overallPercentage,
		formatAggregateMessage(model.jobProgress, aggregate),
		{
			currentFile: model.latestProgressEvent?.current_file,
			etaSeconds: model.latestProgressEvent?.eta_seconds,
		},
	);
	return {
		...model,
		isProcessing: model.jobProgress.size > 0,
		currentStatus: status,
	};
}

export function cloneModel(model: StatusPanelModel): StatusPanelModel {
	return {
		jobProgress: new Map(model.jobProgress),
		queueOrder: [...model.queueOrder],
		queueOrderSet: new Set(model.queueOrderSet),
		lastProgressRenderByKey: new Map(model.lastProgressRenderByKey),
		currentStatus: model.currentStatus,
		isProcessing: model.isProcessing,
		currentWorkKind: model.currentWorkKind,
		latestProgressEvent: model.latestProgressEvent,
		batchCompletionMessageOverride: model.batchCompletionMessageOverride,
	};
}

export function resolveProgressLabel(
	event: ProcessingProgressEvent,
	existingLabel?: string,
): string {
	if (existingLabel) {
		return existingLabel;
	}
	if (typeof event.input_index === 'number') {
		return `Input ${event.input_index + 1}`;
	}
	if (event.job_id) {
		return event.job_id.slice(0, 8);
	}
	const extracted = event.current_file ? extractFilenameFromProgress(event.current_file) : null;
	return extracted ?? 'Processing';
}

export function resolveResultJobKey(
	jobProgress: Map<string, JobProgress>,
	entry: ProcessCommandResult['results'][number],
): string | null {
	if (typeof entry.inputIndex === 'number') {
		return buildJobKey(entry.inputIndex, undefined);
	}
	if (entry.jobId == null) {
		return null;
	}
	const keyedByInput = findJobKeyByJobId(jobProgress, entry.jobId);
	return keyedByInput ?? buildJobKey(undefined, entry.jobId);
}

export function resolveResultLabel(entry: ProcessCommandResult['results'][number]): string {
	if (typeof entry.inputIndex === 'number') {
		return `Input ${entry.inputIndex + 1}`;
	}
	if (entry.jobId) {
		return entry.jobId.slice(0, 8);
	}
	return 'Processing';
}

export function isTerminalProgressStage(stage: EventStage): boolean {
	return (
		stage === STAGES.completed ||
		stage === STAGES.failed ||
		stage === STAGES.skipped ||
		stage === STAGES.cancelled
	);
}

export function toTerminalJobStatus(
	stage: EventStage,
): Exclude<JobStatus, 'processing' | 'queued'> {
	if (stage === STAGES.failed) return 'failed';
	if (stage === STAGES.cancelled) return 'cancelled';
	if (stage === STAGES.skipped) return 'skipped';
	return 'completed';
}

export function isTerminalJobStatus(status: JobStatus): boolean {
	return (
		status === 'completed' || status === 'skipped' || status === 'failed' || status === 'cancelled'
	);
}

function findJobKeyByJobId(jobProgress: Map<string, JobProgress>, jobId: string): string | null {
	for (const [key, job] of jobProgress.entries()) {
		if (job.jobId === jobId) {
			return key;
		}
	}
	return null;
}
