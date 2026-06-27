import {
	STAGES,
	type ProcessingProgressEvent,
	type ProcessingQueueEvent,
} from '../../../types/events';
import type { ProcessCommandResult } from '../../../types/audio';
import { createInitialStatus, type JobStatus } from '../state';
import { areAllBatchJobsTerminal, buildQueueSnapshotState } from './queueState';
import { buildJobKey } from './jobKeys';
import {
	buildBatchCompletionFeedback,
	buildSingleCompletionFeedback,
	cloneModel,
	feedbackFromResult,
	isTerminalJobStatus,
	isTerminalProgressStage,
	recomputeStatus,
	resolveProgressLabel,
	resolveResultJobKey,
	resolveResultLabel,
	toTerminalJobStatus,
} from './stateMachineHelpers';
import {
	BATCH_COMPLETION_HOLD_MS,
	MERGE_SKIP_HOLD_MS,
	PROGRESS_THROTTLE_MS,
	SINGLE_COMPLETION_HOLD_MS,
	type StatusPanelWorkKind,
	type SingleCompletionHoldIntent,
	type StatusPanelCompletionFeedback,
	type StatusPanelIntent,
	type StatusPanelModel,
	type StatusPanelReducerResult,
	workKindFromOperationKind,
} from './stateMachineTypes';

export {
	BATCH_COMPLETION_HOLD_MS,
	MERGE_SKIP_HOLD_MS,
	SINGLE_COMPLETION_HOLD_MS,
	buildBatchCompletionFeedback,
	isTerminalProgressStage,
	workKindFromOperationKind,
};
export type { StatusPanelCompletionFeedback, StatusPanelIntent, StatusPanelModel };

export function createStatusPanelModel(): StatusPanelModel {
	return {
		jobProgress: new Map(),
		queueOrder: [],
		queueOrderSet: new Set(),
		lastProgressRenderByKey: new Map(),
		currentStatus: createInitialStatus(),
		isProcessing: false,
		currentWorkKind: null,
		latestProgressEvent: null,
		batchCompletionMessageOverride: null,
		terminalFeedback: null,
	};
}

export function withBatchCompletionMessage(
	model: StatusPanelModel,
	message: string | null,
): StatusPanelModel {
	return {
		...model,
		batchCompletionMessageOverride: message,
	};
}

export function withCurrentWorkKind(
	model: StatusPanelModel,
	workKind: StatusPanelWorkKind | null,
): StatusPanelModel {
	return {
		...model,
		currentWorkKind: workKind,
	};
}

export function applyQueueSnapshot(
	model: StatusPanelModel,
	event: ProcessingQueueEvent,
	now: number,
): StatusPanelReducerResult {
	const queueSnapshot = buildQueueSnapshotState(event.items, now);
	const next: StatusPanelModel = {
		jobProgress: new Map(queueSnapshot.jobProgress),
		queueOrder: [...queueSnapshot.queueOrder],
		queueOrderSet: new Set(queueSnapshot.queueOrder),
		lastProgressRenderByKey: new Map(),
		currentStatus: model.currentStatus,
		isProcessing: queueSnapshot.queueOrder.length > 0,
		currentWorkKind: workKindFromOperationKind(event.operation_kind),
		latestProgressEvent: model.latestProgressEvent,
		batchCompletionMessageOverride: model.batchCompletionMessageOverride,
		terminalFeedback: null,
	};

	const updated = recomputeStatus(next);
	return { model: updated, intents: [] };
}

export function applyProgress(
	model: StatusPanelModel,
	event: ProcessingProgressEvent,
	now: number,
	options?: { label?: string },
): StatusPanelReducerResult {
	const jobKey = buildJobKey(
		typeof event.input_index === 'number' ? event.input_index : undefined,
		event.job_id,
	);
	const existing = model.jobProgress.get(jobKey);
	const prevStage = existing?.stage;
	const isTerminal = isTerminalProgressStage(event.stage);
	const isStageTransition = prevStage !== undefined && prevStage !== event.stage;
	const lastRender = model.lastProgressRenderByKey.get(jobKey) ?? 0;
	const shouldThrottle =
		!isTerminal && !isStageTransition && now - lastRender < PROGRESS_THROTTLE_MS;

	if (shouldThrottle) {
		return { model, intents: [] };
	}

	const next = cloneModel(model);
	next.currentWorkKind = workKindFromOperationKind(event.operation_kind);
	next.lastProgressRenderByKey.set(jobKey, now);

	const label = options?.label ?? resolveProgressLabel(event, existing?.label);
	const jobStatus: JobStatus = isTerminal ? toTerminalJobStatus(event.stage) : 'processing';
	next.jobProgress.set(jobKey, {
		inputIndex: typeof event.input_index === 'number' ? event.input_index : existing?.inputIndex,
		jobId: event.job_id ?? existing?.jobId,
		label,
		status: jobStatus,
		stage: event.stage,
		percentage: Math.round(event.percentage * 10) / 10,
		message: event.message,
		lastUpdate: now,
	});
	next.latestProgressEvent = event;

	if (typeof event.input_index === 'number') {
		const indexedKey = buildJobKey(event.input_index, undefined);
		if (!next.queueOrderSet.has(indexedKey)) {
			next.queueOrder.push(indexedKey);
			next.queueOrderSet.add(indexedKey);
		}
	}

	const updated = recomputeStatus(next);
	const intents: StatusPanelIntent[] = [];

	if (isTerminal) {
		if (updated.queueOrder.length === 0) {
			intents.push({
				kind: 'single-completion-hold',
				jobKey,
				terminalStage: event.stage,
				message: event.message,
				holdMs: SINGLE_COMPLETION_HOLD_MS,
			});
		} else if (areAllBatchJobsTerminal(updated.queueOrder, updated.jobProgress)) {
			intents.push({
				kind: 'batch-completion-hold',
				holdMs: BATCH_COMPLETION_HOLD_MS,
			});
		}
	}

	return { model: updated, intents };
}

export function reconcileProcessResult(
	model: StatusPanelModel,
	result: ProcessCommandResult,
	now: number,
	options?: { mergeOutputLabel?: string },
): StatusPanelReducerResult {
	if (result.jobType === 'merge') {
		const skippedMergeEntry = result.results.find((entry) => entry.status === 'skipped');
		if (skippedMergeEntry) {
			const next = cloneModel(model);
			const mergeKey = buildJobKey(undefined, skippedMergeEntry.jobId ?? undefined);
			const label = options?.mergeOutputLabel ?? 'Merge output';
			next.jobProgress.set(mergeKey, {
				inputIndex: skippedMergeEntry.inputIndex ?? undefined,
				jobId: skippedMergeEntry.jobId ?? undefined,
				label,
				status: 'skipped',
				stage: STAGES.skipped,
				percentage: 100,
				message: skippedMergeEntry.message,
				lastUpdate: now,
			});
			const updated = recomputeStatus(next);
			return {
				model: updated,
				intents: [
					{
						kind: 'merge-skip-hold',
						jobKey: mergeKey,
						message: skippedMergeEntry.message,
						holdMs: MERGE_SKIP_HOLD_MS,
					},
				],
			};
		}
	}

	const next = cloneModel(model);
	// The backend owns the terminal verdict (`classify_run_terminal` →
	// `RunTerminalClass`); carry it so the completion toast renders backend truth
	// instead of re-deriving precedence from per-job statuses. Set unconditionally
	// — an all-success run repairs no rows below but still needs its verdict.
	next.terminalFeedback = feedbackFromResult(result);
	let didUpdate = false;
	for (const entry of result.results) {
		if (entry.status === 'success') {
			continue;
		}

		const key = resolveResultJobKey(next.jobProgress, entry);
		if (!key) {
			continue;
		}

		const existing = next.jobProgress.get(key);
		next.jobProgress.set(key, {
			inputIndex: existing?.inputIndex ?? entry.inputIndex ?? undefined,
			jobId: entry.jobId ?? existing?.jobId,
			label: existing?.label ?? resolveResultLabel(entry),
			status: entry.status,
			stage: entry.status,
			percentage: 100,
			message: entry.message,
			lastUpdate: now,
		});
		didUpdate = true;
	}

	if (!didUpdate) {
		return { model: next, intents: [] };
	}

	const updated = recomputeStatus(next);
	const intents: StatusPanelIntent[] = [];

	if (
		updated.queueOrder.length > 0 &&
		areAllBatchJobsTerminal(updated.queueOrder, updated.jobProgress)
	) {
		intents.push({
			kind: 'batch-completion-hold',
			holdMs: BATCH_COMPLETION_HOLD_MS,
		});
	}

	return { model: updated, intents };
}

export function applyCancellation(model: StatusPanelModel, now: number): StatusPanelReducerResult {
	const next = cloneModel(model);
	let didUpdate = false;

	for (const [jobKey, job] of next.jobProgress.entries()) {
		if (isTerminalJobStatus(job.status)) {
			continue;
		}
		next.jobProgress.set(jobKey, {
			...job,
			status: 'cancelled',
			stage: STAGES.cancelled,
			message: 'Processing was cancelled.',
			lastUpdate: now,
		});
		didUpdate = true;
	}

	if (!didUpdate) {
		return { model, intents: [] };
	}

	const updated = recomputeStatus(next);
	const intents: StatusPanelIntent[] = [];

	if (updated.queueOrder.length > 0) {
		intents.push({
			kind: 'batch-completion-hold',
			holdMs: BATCH_COMPLETION_HOLD_MS,
		});
	} else {
		const firstEntry = updated.jobProgress.keys().next().value;
		if (firstEntry) {
			intents.push({
				kind: 'single-completion-hold',
				jobKey: firstEntry,
				terminalStage: STAGES.cancelled,
				message: 'Processing was cancelled.',
				holdMs: SINGLE_COMPLETION_HOLD_MS,
			});
		}
	}

	return { model: updated, intents };
}

export function completeBatchCompletionHold(model: StatusPanelModel): {
	model: StatusPanelModel;
	feedback: StatusPanelCompletionFeedback;
} {
	return {
		model: createStatusPanelModel(),
		feedback: buildBatchCompletionFeedback(model),
	};
}

export function completeSingleCompletionHold(
	model: StatusPanelModel,
	jobKey: string,
	event: Pick<SingleCompletionHoldIntent, 'terminalStage' | 'message'>,
): {
	model: StatusPanelModel;
	feedback: StatusPanelCompletionFeedback | null;
} {
	const next = cloneModel(model);
	next.jobProgress.delete(jobKey);

	if (next.jobProgress.size === 0) {
		return {
			model: createStatusPanelModel(),
			feedback: buildSingleCompletionFeedback(event),
		};
	}

	return {
		model: recomputeStatus(next),
		feedback: null,
	};
}

export function completeMergeSkipHold(
	model: StatusPanelModel,
	jobKey: string,
	message: string,
): {
	model: StatusPanelModel;
	feedback: StatusPanelCompletionFeedback | null;
} {
	const next = cloneModel(model);
	next.jobProgress.delete(jobKey);

	if (next.jobProgress.size === 0) {
		return {
			model: createStatusPanelModel(),
			feedback: { kind: 'info', message },
		};
	}

	return {
		model: recomputeStatus(next),
		feedback: null,
	};
}

export function resetStatusPanelModel(): StatusPanelModel {
	return createStatusPanelModel();
}
