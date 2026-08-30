import type {
	OperationListSnapshot,
	OperationSnapshot,
	WorkOperationStatus,
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

export function isTerminalOperationStatus(status: WorkOperationStatus): boolean {
	return TERMINAL_OPERATION_STATUSES.has(status);
}

export function replaceOperations(
	_model: WorkCenterModel,
	list: OperationListSnapshot,
): WorkCenterModel {
	return {
		operations: [...list.operations].sort(sortByStatusThenSequenceDesc),
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
	next.sort(sortByStatusThenSequenceDesc);
	return { operations: next };
}

function statusDisplayBucket(status: WorkOperationStatus): 0 | 1 | 2 {
	if (isTerminalOperationStatus(status)) return 2;
	if (status === 'accepted') return 1;
	return 0;
}

function sortByStatusThenSequenceDesc(left: OperationSnapshot, right: OperationSnapshot): number {
	const bucketDiff = statusDisplayBucket(left.status) - statusDisplayBucket(right.status);
	if (bucketDiff !== 0) return bucketDiff;
	return right.sequence - left.sequence;
}
