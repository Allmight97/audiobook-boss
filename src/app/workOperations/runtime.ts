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
} from '../../ui/remoteSource';
import {
	isTerminalOperationStatus,
	replaceOperations,
	upsertOperation,
	type WorkCenterModel,
} from './model';
import { toUserMessage } from '../../lib/tauri/appError';
import { createSubscriptionGroup, type SubscriptionGroup } from '../../lib/tauri/subscriptionGroup';

export type WorkOperationsView = {
	readonly initialized: boolean;
	readonly operations: ReadonlyArray<OperationSnapshot>;
	readonly cancelPendingByOperationId: Readonly<Record<string, boolean>>;
	readonly errorMessage: string | null;
};

interface WorkCenterState extends WorkCenterModel {
	initialized: boolean;
	cancelPendingByOperationId: Record<string, boolean>;
	errorMessage: string | null;
}

export const PURGED_OPERATION_TOMBSTONE_CAP = 64;

function emptyWorkCenterState(): WorkCenterState {
	return {
		initialized: false,
		operations: [],
		cancelPendingByOperationId: {},
		errorMessage: null,
	};
}

export function emptyWorkOperationsView(): WorkOperationsView {
	return {
		initialized: false,
		operations: [],
		cancelPendingByOperationId: {},
		errorMessage: null,
	};
}

export type WorkOperationsSession = {
	readonly view: () => WorkOperationsView;
	initialize(): Promise<void>;
	dispose(): void;
	applyOperationSnapshot(snapshot: OperationSnapshot): void;
	cancel(operationId: OperationId): Promise<void>;
	openSource(child: { sourcePath?: string | null }): Promise<void>;
};

function isTauriRuntimeAvailable(): boolean {
	return (
		typeof window === 'undefined' ||
		typeof (window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ !==
			'undefined'
	);
}

export function createWorkOperationsSession(
	publish: (view: WorkOperationsView) => void,
): WorkOperationsSession {
	const state = emptyWorkCenterState();
	let initializationPromise: Promise<void> | null = null;
	let subscriptions: SubscriptionGroup | null = null;
	const purgedOperationIds = new Set<string>();
	const purgedOperationOrder: string[] = [];

	function snapshot(): WorkOperationsView {
		return {
			initialized: state.initialized,
			operations: state.operations,
			cancelPendingByOperationId: state.cancelPendingByOperationId,
			errorMessage: state.errorMessage,
		};
	}

	function commit(): void {
		publish(snapshot());
	}

	function markOperationPurged(operationId: string): void {
		purgedOperationIds.add(operationId);
		purgedOperationOrder.push(operationId);
		if (purgedOperationOrder.length > PURGED_OPERATION_TOMBSTONE_CAP) {
			const oldest = purgedOperationOrder.shift();
			if (oldest !== undefined) purgedOperationIds.delete(oldest);
		}
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

	function applyOperationSnapshot(next: OperationSnapshot): void {
		const model = upsertOperation(state, next);
		state.operations = model.operations;
		commit();
		void purgeRemoteSessionsForTerminalOperation(next);
	}

	function applyOperationListSnapshot(list: OperationListSnapshot): void {
		const model = replaceOperations(state, list);
		state.operations = model.operations;
		commit();
		for (const operation of state.operations) {
			void purgeRemoteSessionsForTerminalOperation(operation);
		}
	}

	return {
		view: snapshot,
		initialize() {
			if (initializationPromise) return initializationPromise;
			if (!isTauriRuntimeAvailable()) {
				state.initialized = true;
				state.errorMessage = null;
				commit();
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
				if (group.disposed) {
					return;
				}
				applyOperationListSnapshot(list);
				state.initialized = true;
				state.errorMessage = null;
				commit();
			})().catch((error) => {
				group.dispose();
				if (subscriptions === group) {
					subscriptions = null;
				}
				state.errorMessage = `Failed to initialize Work Center: ${toUserMessage(error)}`;
				initializationPromise = null;
				commit();
				throw error;
			});

			return initializationPromise;
		},
		dispose() {
			subscriptions?.dispose();
			subscriptions = null;
			initializationPromise = null;
			state.initialized = false;
			state.operations = [];
			state.cancelPendingByOperationId = {};
			state.errorMessage = null;
			purgedOperationIds.clear();
			purgedOperationOrder.length = 0;
			commit();
		},
		applyOperationSnapshot,
		async cancel(operationId) {
			state.cancelPendingByOperationId = {
				...state.cancelPendingByOperationId,
				[operationId]: true,
			};
			commit();
			try {
				const next = await tauriClient.cancelWorkOperation(operationId);
				applyOperationSnapshot(next);
			} catch (error) {
				state.errorMessage = `Failed to cancel operation: ${toUserMessage(error)}`;
				commit();
			} finally {
				const next = { ...state.cancelPendingByOperationId };
				delete next[operationId];
				state.cancelPendingByOperationId = next;
				commit();
			}
		},
		async openSource(child) {
			if (!child.sourcePath) return;
			try {
				await tauriClient.openPath(child.sourcePath);
			} catch (error) {
				state.errorMessage = `Failed to open source file: ${toUserMessage(error)}`;
				commit();
			}
		},
	};
}
