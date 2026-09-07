import type {
	OperationListSnapshot,
	OperationSnapshot,
	WorkOperationStatus,
} from '../../types/workRuntime';

export interface WorkCenterModel {
	membershipRevision: number;
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
	model: WorkCenterModel,
	list: OperationListSnapshot,
): WorkCenterModel {
	if (list.membershipRevision < model.membershipRevision) return model;
	const previous = new Map(model.operations.map((operation) => [operation.operationId, operation]));
	const operations = list.operations.map((incoming) => {
		const current = previous.get(incoming.operationId);
		previous.delete(incoming.operationId);
		return current && current.revision > incoming.revision ? current : incoming;
	});
	// An event can announce new work after this list was captured.
	for (const operation of previous.values()) {
		if (operation.createdRevision > list.membershipRevision) operations.push(operation);
	}
	return {
		membershipRevision: list.membershipRevision,
		operations: operations.sort(sortByStatusThenSequenceDesc),
	};
}

export function upsertOperation(
	model: WorkCenterModel,
	snapshot: OperationSnapshot,
): WorkCenterModel {
	const current = model.operations.find(
		(operation) => operation.operationId === snapshot.operationId,
	);
	if (
		current
			? snapshot.revision <= current.revision
			: snapshot.createdRevision <= model.membershipRevision
	)
		return model;
	const next = model.operations.filter(
		(operation) => operation.operationId !== snapshot.operationId,
	);
	next.push(snapshot);
	next.sort(sortByStatusThenSequenceDesc);
	return { membershipRevision: model.membershipRevision, operations: next };
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
