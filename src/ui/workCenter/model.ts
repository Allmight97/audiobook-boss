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

function sortBySequenceDesc(left: OperationSnapshot, right: OperationSnapshot): number {
	return right.sequence - left.sequence;
}
