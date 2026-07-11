import type {
	ChildJobStatus,
	OperationListSnapshot,
	OperationSnapshot,
	ProgressSnapshot,
	WorkOperationStatus,
} from '../../types/workRuntime';

export interface WorkCenterModel {
	operations: OperationSnapshot[];
}

export type WorkActivityStatus = 'queued' | 'running' | 'done' | 'cancelled' | 'failed';

export type WorkActivity = {
	status: WorkActivityStatus;
	progress: ProgressSnapshot;
	sequence: number;
};

export type WorkOperationCounts = {
	running: number;
	queued: number;
	done: number;
};

const TERMINAL_OPERATION_STATUSES = new Set<WorkOperationStatus>([
	'completed',
	'cancelled',
	'failed',
	'mixed',
]);

export function isTerminalOperationStatus(status: WorkOperationStatus): boolean {
	return TERMINAL_OPERATION_STATUSES.has(status);
}

export function deriveWorkOperationCounts(
	operations: readonly OperationSnapshot[],
): WorkOperationCounts {
	return operations.reduce<WorkOperationCounts>(
		(counts, operation) => {
			if (isTerminalOperationStatus(operation.status)) {
				counts.done += 1;
			} else if (operation.status === 'accepted') {
				counts.queued += 1;
			} else {
				counts.running += 1;
			}
			return counts;
		},
		{ running: 0, queued: 0, done: 0 },
	);
}

function workActivityStatusFromOperation(status: WorkOperationStatus): WorkActivityStatus {
	if (!isTerminalOperationStatus(status)) {
		return status === 'accepted' ? 'queued' : 'running';
	}
	if (status === 'completed') return 'done';
	return status === 'cancelled' ? 'cancelled' : 'failed';
}

function workActivityStatusFromChild(status: ChildJobStatus): WorkActivityStatus {
	if (status === 'queued') return 'queued';
	if (status === 'running') return 'running';
	if (status === 'cancelled') return 'cancelled';
	return status === 'completed' || status === 'skipped' ? 'done' : 'failed';
}

/**
 * Projects backend-authored snapshots onto the input IDs that the file table
 * owns. When a later operation mentions an input, it supersedes earlier work.
 */
export function deriveWorkActivityByInputId(
	operations: readonly OperationSnapshot[],
): ReadonlyMap<string, WorkActivity> {
	const activityByInputId = new Map<string, WorkActivity>();

	function setIfLatest(inputId: string, activity: WorkActivity): void {
		const current = activityByInputId.get(inputId);
		if (!current || activity.sequence > current.sequence) {
			activityByInputId.set(inputId, activity);
		}
	}

	for (const operation of operations) {
		const childrenWithInputIds = operation.children.filter(
			(child): child is typeof child & { inputId: string } => child.inputId != null,
		);
		for (const child of childrenWithInputIds) {
			setIfLatest(child.inputId, {
				status: workActivityStatusFromChild(child.status),
				progress: child.progress,
				sequence: operation.sequence,
			});
		}

		if (operation.children.some((child) => child.inputId == null) || operation.children.length === 0) {
			for (const inputId of operation.sourceInputIds) {
				setIfLatest(inputId, {
					status: workActivityStatusFromOperation(operation.status),
					progress: operation.progress,
					sequence: operation.sequence,
				});
			}
		}
	}

	return activityByInputId;
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
