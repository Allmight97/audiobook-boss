import type {
	OperationId,
	OperationSnapshot,
	WorkOperationsSnapshot,
	WorkOperationsUpdate,
} from '../../lib/generated/tauri';

export interface WorkCenterModel {
	revision: number;
	order: OperationId[];
	byId: ReadonlyMap<OperationId, OperationSnapshot>;
	concurrencyLocked: boolean;
}

export function emptyWorkCenterModel(): WorkCenterModel {
	return { revision: 0, order: [], byId: new Map(), concurrencyLocked: false };
}

export function applyWorkOperations(
	model: WorkCenterModel,
	incoming: WorkOperationsSnapshot | WorkOperationsUpdate,
): WorkCenterModel {
	const snapshots =
		'operations' in incoming ? incoming.operations : incoming.changed ? [incoming.changed] : [];
	const newer = incoming.revision > model.revision;
	const order = newer ? incoming.order : model.order;
	const concurrencyLocked = newer ? incoming.concurrencyLocked : model.concurrencyLocked;
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
	return { revision: newer ? incoming.revision : model.revision, order, byId, concurrencyLocked };
}

export function visibleOperations(model: WorkCenterModel): OperationSnapshot[] {
	return model.order.flatMap((id) => {
		const snapshot = model.byId.get(id);
		return snapshot ? [snapshot] : [];
	});
}
