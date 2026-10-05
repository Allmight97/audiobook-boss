import type {
	OperationId,
	OperationSnapshot,
	WorkOperationsSnapshot,
	WorkOperationsUpdate,
} from '../../types/workRuntime';

/**
 * The engine numbers every Work Center change and sends the display order with
 * it. Updates may arrive in any order, so this keeps the newest snapshot of
 * each operation and the order with the highest revision; operations outside
 * that order are gone.
 */
export interface WorkCenterModel {
	revision: number;
	order: OperationId[];
	byId: ReadonlyMap<OperationId, OperationSnapshot>;
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

/** A cancel reply: one operation's snapshot, with no newer order. */
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
