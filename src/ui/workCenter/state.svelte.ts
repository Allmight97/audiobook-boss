import { tauriClient } from '../../lib/tauri/client';
import { EVENTS } from '../../types/events';
import type {
	OperationId,
	OperationListSnapshot,
	OperationSnapshot,
} from '../../types/workRuntime';
import {
	purgeRemoteSourceSessionsForInputIds,
	releaseRemoteSourceSessionRetainers,
} from '../remoteSource';
import {
	deriveWorkActivityByInputId,
	isTerminalOperationStatus,
	replaceOperations,
	upsertOperation,
	type WorkCenterModel,
} from './model';
import { toUserMessage } from '../../lib/tauri/appError';
import { createSubscriptionGroup, type SubscriptionGroup } from '../../lib/tauri/subscriptionGroup';

interface WorkCenterState extends WorkCenterModel {
	initialized: boolean;
	cancelPendingByOperationId: Record<string, boolean>;
	errorMessage: string | null;
}

export const workCenterState = $state<WorkCenterState>({
	initialized: false,
	operations: [],
	cancelPendingByOperationId: {},
	errorMessage: null,
});

export const workCenterClock = $state({ nowMs: Date.now() });

let initializationPromise: Promise<void> | null = null;
let subscriptions: SubscriptionGroup | null = null;

// Bounded FIFO tombstone of operation ids already purged: an operation's
// terminal snapshot arrives from both the single-snapshot and list-snapshot
// events, and the list snapshot keeps resending it on every unrelated
// operation's event until the backend prunes it (see the Rust runtime's
// terminal-operation cap, `TERMINAL_OPERATIONS_CAP` in
// `src-tauri/src/work_runtime/state.rs`, currently 20). This cap MUST stay
// above that backend cap: an id can only legitimately reappear in a list
// snapshot while the backend still retains it, so keeping this tombstone
// larger than the backend's retention window guarantees an evicted id can
// never be re-delivered and mistakenly re-trigger a release.
const PURGED_OPERATION_TOMBSTONE_CAP = 64;
const purgedOperationIds = new Set<string>();
const purgedOperationOrder: string[] = [];

function markOperationPurged(operationId: string): void {
	purgedOperationIds.add(operationId);
	purgedOperationOrder.push(operationId);
	if (purgedOperationOrder.length > PURGED_OPERATION_TOMBSTONE_CAP) {
		const oldest = purgedOperationOrder.shift();
		if (oldest !== undefined) purgedOperationIds.delete(oldest);
	}
}

export function startWorkCenterClock(): () => void {
	workCenterClock.nowMs = Date.now();
	const intervalId = window.setInterval(() => {
		workCenterClock.nowMs = Date.now();
	}, 30_000);
	return () => window.clearInterval(intervalId);
}

export function initializeWorkCenter(): Promise<void> {
	if (initializationPromise) return initializationPromise;
	if (!isTauriRuntimeAvailable()) {
		workCenterState.initialized = true;
		workCenterState.errorMessage = null;
		return Promise.resolve();
	}

	const group = createSubscriptionGroup();
	subscriptions = group;
	initializationPromise = (async () => {
		await group.add(
			tauriClient.listen(EVENTS.WORK_OPERATION_SNAPSHOT, ({ payload }) => {
				applyOperationSnapshot(payload.snapshot);
			}),
		);
		await group.add(
			tauriClient.listen(EVENTS.WORK_OPERATION_LIST_SNAPSHOT, ({ payload }) => {
				applyOperationListSnapshot({ operations: payload.operations });
			}),
		);

		const list = await tauriClient.listWorkOperations();
		// A dispose during initialization wins: do not mark initialized or apply state.
		if (group.disposed) {
			return;
		}
		applyOperationListSnapshot(list);
		workCenterState.initialized = true;
		workCenterState.errorMessage = null;
	})().catch((error) => {
		group.dispose();
		if (subscriptions === group) {
			subscriptions = null;
		}
		workCenterState.errorMessage = `Failed to initialize Work Center: ${toUserMessage(error)}`;
		initializationPromise = null;
		throw error;
	});

	return initializationPromise;
}

function isTauriRuntimeAvailable(): boolean {
	return (
		typeof window === 'undefined' ||
		typeof (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !==
			'undefined'
	);
}

export function disposeWorkCenter(): void {
	subscriptions?.dispose();
	subscriptions = null;
	initializationPromise = null;
	workCenterState.initialized = false;
}

export function applyOperationSnapshot(snapshot: OperationSnapshot): void {
	const model = upsertOperation(workCenterState, snapshot);
	workCenterState.operations = model.operations;
	void purgeRemoteSessionsForTerminalOperation(snapshot);
}

export function applyOperationListSnapshot(list: OperationListSnapshot): void {
	const model = replaceOperations(workCenterState, list);
	workCenterState.operations = model.operations;
	for (const operation of workCenterState.operations) {
		void purgeRemoteSessionsForTerminalOperation(operation);
	}
}

export function readWorkActivityByInputId() {
	return deriveWorkActivityByInputId(workCenterState.operations);
}

export async function cancelWorkOperation(operationId: OperationId): Promise<void> {
	workCenterState.cancelPendingByOperationId = {
		...workCenterState.cancelPendingByOperationId,
		[operationId]: true,
	};
	try {
		const snapshot = await tauriClient.cancelWorkOperation(operationId);
		applyOperationSnapshot(snapshot);
	} catch (error) {
		workCenterState.errorMessage = `Failed to cancel operation: ${toUserMessage(error)}`;
	} finally {
		const next = { ...workCenterState.cancelPendingByOperationId };
		delete next[operationId];
		workCenterState.cancelPendingByOperationId = next;
	}
}

export async function openChildSource(child: { sourcePath?: string | null }): Promise<void> {
	if (!child.sourcePath) return;
	await tauriClient.openPath(child.sourcePath);
}

async function purgeRemoteSessionsForTerminalOperation(
	operation: OperationSnapshot,
): Promise<void> {
	if (!isTerminalOperationStatus(operation.status)) return;
	if (purgedOperationIds.has(operation.operationId)) return;
	markOperationPurged(operation.operationId);

	const operationInputIds =
		operation.sourceInputIds.length > 0
			? operation.sourceInputIds
			: operation.children
					.map((child) => child.inputId)
					.filter((inputId): inputId is string => Boolean(inputId));
	const pendingPurgeInputIds = releaseRemoteSourceSessionRetainers(operationInputIds);
	const completedInputIds =
		operation.status === 'completed'
			? operationInputIds
			: operation.children
					.filter((child) => child.status === 'completed')
					.map((child) => child.inputId)
					.filter((inputId): inputId is string => Boolean(inputId));
	const purgeInputIds = Array.from(new Set([...completedInputIds, ...pendingPurgeInputIds]));
	if (purgeInputIds.length === 0) {
		return;
	}

	await purgeRemoteSourceSessionsForInputIds(purgeInputIds);
}
