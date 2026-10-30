import type {
	OperationId,
	OperationSnapshot,
	WorkOperationsSnapshot,
	WorkOperationsUpdate,
} from '../../types/workRuntime';

export interface WorkCenterModel {
	revision: number;
	order: OperationId[];
	byId: ReadonlyMap<OperationId, OperationSnapshot>;
}

/** Work runs now, a cancel included until it lands; only running work shows motion. */
export function isWorkRunning(status: OperationSnapshot['status']): boolean {
	return status === 'running' || status === 'cancelling';
}

export function emptyWorkCenterModel(): WorkCenterModel {
	return { revision: 0, order: [], byId: new Map() };
}

export function applyWorkOperations(
	model: WorkCenterModel,
	incoming: WorkOperationsSnapshot | WorkOperationsUpdate,
): WorkCenterModel {
	const snapshots = 'changed' in incoming ? [incoming.changed] : incoming.operations;
	const newer = incoming.revision > model.revision;
	const order = newer ? incoming.order : model.order;
	const listed = new Set(order);
	const byId = new Map<OperationId, OperationSnapshot>();
	for (const id of listed) {
		const kept = model.byId.get(id);
		if (kept) byId.set(id, kept);
	}
	let changed = newer;
	for (const snapshot of snapshots) {
		if (!listed.has(snapshot.operationId)) continue;
		const kept = byId.get(snapshot.operationId);
		if (kept && kept.revision >= snapshot.revision) continue;
		byId.set(snapshot.operationId, snapshot);
		changed = true;
	}
	if (!changed) return model;
	return { revision: newer ? incoming.revision : model.revision, order, byId };
}

export function applyOperationSnapshot(
	model: WorkCenterModel,
	snapshot: OperationSnapshot,
): WorkCenterModel {
	return applyWorkOperations(model, {
		revision: model.revision,
		order: model.order,
		changed: snapshot,
	});
}

export function visibleOperations(model: WorkCenterModel): OperationSnapshot[] {
	return model.order.flatMap((id) => {
		const snapshot = model.byId.get(id);
		return snapshot ? [snapshot] : [];
	});
}
