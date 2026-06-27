import type { ProcessingProgressEvent, EventStage, OperationKind } from '../../../types/events';
import type { JobProgress, ProcessingStatus } from '../state';

export const SINGLE_COMPLETION_HOLD_MS = 2000;
export const BATCH_COMPLETION_HOLD_MS = 2000;
export const MERGE_SKIP_HOLD_MS = 1500;
export const PROGRESS_THROTTLE_MS = 1000;

export type CompletionFeedbackKind = 'success' | 'error' | 'info';
export type StatusPanelWorkKind = 'merge' | 'batch' | 'remoteAcquisition' | 'metadataSave';

export function workKindFromOperationKind(operationKind: OperationKind): StatusPanelWorkKind {
	switch (operationKind) {
		case 'processingMerge':
			return 'merge';
		case 'processingBatch':
			return 'batch';
		case 'remoteAcquisition':
			return 'remoteAcquisition';
		case 'metadataSave':
			return 'metadataSave';
	}
}

export interface StatusPanelCompletionFeedback {
	kind: CompletionFeedbackKind;
	message: string;
}

export interface SingleCompletionHoldIntent {
	kind: 'single-completion-hold';
	jobKey: string;
	terminalStage: EventStage;
	message: string;
	holdMs: number;
}

export interface BatchCompletionHoldIntent {
	kind: 'batch-completion-hold';
	holdMs: number;
}

export interface MergeSkipHoldIntent {
	kind: 'merge-skip-hold';
	jobKey: string;
	message: string;
	holdMs: number;
}

export type StatusPanelIntent =
	| SingleCompletionHoldIntent
	| BatchCompletionHoldIntent
	| MergeSkipHoldIntent;

export interface StatusPanelReducerResult {
	model: StatusPanelModel;
	intents: StatusPanelIntent[];
}

export interface StatusPanelModel {
	jobProgress: Map<string, JobProgress>;
	queueOrder: string[];
	queueOrderSet: Set<string>;
	lastProgressRenderByKey: Map<string, number>;
	currentStatus: ProcessingStatus;
	isProcessing: boolean;
	currentWorkKind: StatusPanelWorkKind | null;
	latestProgressEvent: ProcessingProgressEvent | null;
	batchCompletionMessageOverride: string | null;
	// Backend-owned terminal verdict for the reconciled run (kind + default
	// message), mapped once from `RunTerminalClass`. The UI renders this rather
	// than re-deriving terminal precedence from per-job statuses. `null` until a
	// result is reconciled (e.g. a purely local foreground cancellation).
	terminalFeedback: StatusPanelCompletionFeedback | null;
	// True once a user-initiated local cancellation has settled this run. The
	// foreground/preview lane is uncancellable at the backend, so its result still
	// arrives and reconciles; this latch keeps the user's cancellation verdict from
	// being overwritten by that late backend verdict (a success toast after cancel).
	cancellationLatched: boolean;
}
