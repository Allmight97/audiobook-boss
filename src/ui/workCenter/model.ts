import type { ProcessingProgressEvent } from '../../types/events';
import type {
	ChildJobSnapshot,
	ChildJobStatus,
	OperationListSnapshot,
	OperationSnapshot,
	WorkOperationStatus,
	WorkProgressStage,
} from '../../types/workRuntime';

export interface WorkCenterModel {
	operations: OperationSnapshot[];
}

const TERMINAL_OPERATION_STATUSES = new Set<WorkOperationStatus>([
	'completed',
	'cancelled',
	'failed',
	'mixed',
]);
const TERMINAL_CHILD_STATUSES = new Set<ChildJobStatus>([
	'completed',
	'skipped',
	'cancelled',
	'failed',
]);

export function isTerminalOperationStatus(status: WorkOperationStatus): boolean {
	return TERMINAL_OPERATION_STATUSES.has(status);
}

export function replaceOperations(
	_model: WorkCenterModel,
	list: OperationListSnapshot,
): WorkCenterModel {
	return {
		operations: [...list.operations].sort(sortBySequenceDesc),
	};
}

export function upsertOperation(
	model: WorkCenterModel,
	snapshot: OperationSnapshot,
): WorkCenterModel {
	const next = model.operations.filter(
		(operation) => operation.operationId !== snapshot.operationId,
	);
	next.push(snapshot);
	next.sort(sortBySequenceDesc);
	return { operations: next };
}

export function applyProgress(
	model: WorkCenterModel,
	event: ProcessingProgressEvent,
): WorkCenterModel {
	const operationId = event.operation_id;
	if (!operationId) return model;

	let changed = false;
	const operations = model.operations.map((operation) => {
		if (operation.operationId !== operationId) return operation;
		changed = true;
		return applyProgressToOperation(operation, event);
	});
	return changed ? { operations } : model;
}

function applyProgressToOperation(
	operation: OperationSnapshot,
	event: ProcessingProgressEvent,
): OperationSnapshot {
	const childScopedBatchEvent =
		operation.children.length > 1 && typeof event.input_index === 'number';
	const children = operation.children.map((child) => {
		if (!progressMatchesChild(child, event, operation.children.length)) {
			return child;
		}
		const status = childStatusFromEvent(event.stage);
		return {
			...child,
			status,
			jobId: event.job_id ?? child.jobId,
			progress: {
				...child.progress,
				stage: workStageFromEvent(event.stage),
				percentage: event.percentage,
				message: event.message,
				etaSeconds: event.eta_seconds,
			},
			cancellable: status === 'running' && Boolean(event.job_id),
			message: event.message,
		};
	});
	const nextStatus = operationStatusFromEvent(event, operation.status, childScopedBatchEvent);
	const nextProgress = {
		...operation.progress,
		stage: childScopedBatchEvent
			? inFlightStage(operation.progress.stage)
			: workStageFromEvent(event.stage),
		percentage: childScopedBatchEvent ? aggregateChildProgress(children) : event.percentage,
		message: event.message,
		etaSeconds: event.eta_seconds,
	};

	return {
		...operation,
		status: nextStatus,
		progress: nextProgress,
		children,
	};
}

function progressMatchesChild(
	child: ChildJobSnapshot,
	event: ProcessingProgressEvent,
	childCount: number,
): boolean {
	if (event.job_id && child.jobId === event.job_id) return true;
	if (typeof event.input_index === 'number') {
		return child.inputIndex === event.input_index;
	}
	return childCount === 1;
}

function operationStatusFromEvent(
	event: ProcessingProgressEvent,
	current: WorkOperationStatus,
	childScopedBatchEvent: boolean,
): WorkOperationStatus {
	if (isTerminalOperationStatus(current)) return current;
	if (childScopedBatchEvent) return 'running';
	if (event.stage === 'failed') return 'failed';
	if (event.stage === 'cancelled') return 'cancelled';
	return 'running';
}

function childStatusFromEvent(stage: ProcessingProgressEvent['stage']): ChildJobStatus {
	if (stage === 'completed') return 'completed';
	if (stage === 'skipped') return 'skipped';
	if (stage === 'failed') return 'failed';
	if (stage === 'cancelled') return 'cancelled';
	return 'running';
}

function workStageFromEvent(stage: ProcessingProgressEvent['stage']): WorkProgressStage {
	if (stage === 'analyzing') return 'analyzing';
	if (stage === 'converting') return 'converting';
	if (stage === 'writing') return 'writing';
	if (stage === 'completed' || stage === 'skipped') return 'complete';
	if (stage === 'failed') return 'failed';
	return 'cancelled';
}

function inFlightStage(current: WorkProgressStage): WorkProgressStage {
	if (current === 'pending' || current === 'analyzing') return 'converting';
	return current;
}

function aggregateChildProgress(children: ChildJobSnapshot[]): number {
	if (children.length === 0) return 0;
	const total = children.reduce((sum, child) => {
		const percentage = TERMINAL_CHILD_STATUSES.has(child.status) ? 100 : child.progress.percentage;
		return sum + Math.min(100, Math.max(0, percentage));
	}, 0);
	return total / children.length;
}

function sortBySequenceDesc(left: OperationSnapshot, right: OperationSnapshot): number {
	return right.sequence - left.sequence;
}
