import { tauriClient } from '../../lib/tauri/client';
import { EVENTS } from '../../types/events';
import type { OperationId, OperationSnapshot, WorkOperationsUpdate } from '../../types/workRuntime';
import {
	applyOperationSnapshot as mergeOperationSnapshot,
	applyWorkOperations,
	emptyWorkCenterModel,
	visibleOperations,
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

interface WorkCenterState {
	model: WorkCenterModel;
	initialized: boolean;
	cancelPendingByOperationId: Record<string, boolean>;
	errorMessage: string | null;
}

/** A later successful cancel clears an earlier cancel failure. */
const CANCEL_ERROR_PREFIX = 'Failed to cancel';

function emptyWorkCenterState(): WorkCenterState {
	return {
		model: emptyWorkCenterModel(),
		initialized: false,
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
	applyUpdate(update: WorkOperationsUpdate): void;
	cancel(operationId: OperationId, childJobId?: string): Promise<void>;
	revealOutput(child: { outputPath?: string | null }): Promise<void>;
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
	let generation = 0;

	function snapshot(): WorkOperationsView {
		return {
			initialized: state.initialized,
			operations: visibleOperations(state.model),
			cancelPendingByOperationId: state.cancelPendingByOperationId,
			errorMessage: state.errorMessage,
		};
	}

	function commit(): void {
		publish(snapshot());
	}

	async function sendCancel(operationId: OperationId, childJobId?: string): Promise<void> {
		const started = generation;
		try {
			const next = await tauriClient.cancelWorkOperation(operationId, childJobId);
			if (started !== generation) return;
			if (state.errorMessage?.startsWith(CANCEL_ERROR_PREFIX)) {
				state.errorMessage = null;
				commit();
			}
			applyOperationSnapshot(next);
		} catch (error) {
			if (started !== generation) return;
			state.errorMessage = `${CANCEL_ERROR_PREFIX} ${childJobId ? 'title' : 'operation'}: ${toUserMessage(error)}`;
			commit();
		}
	}

	function apply(model: WorkCenterModel): void {
		if (model === state.model) return;
		state.model = model;
		commit();
	}

	function applyOperationSnapshot(next: OperationSnapshot): void {
		apply(mergeOperationSnapshot(state.model, next));
	}

	function applyUpdate(update: WorkOperationsUpdate): void {
		apply(applyWorkOperations(state.model, update));
	}

	return {
		view: snapshot,
		initialize() {
			if (initializationPromise !== null) return initializationPromise;
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
					tauriClient.listen(EVENTS.WORK_OPERATIONS_UPDATE, ({ payload }) => applyUpdate(payload)),
				);

				const list = await tauriClient.listWorkOperations();
				if (group.disposed) {
					return;
				}
				apply(applyWorkOperations(state.model, list));
				state.initialized = true;
				state.errorMessage = null;
				commit();
			})().catch((error) => {
				if (group.disposed || subscriptions !== group) return;
				group.dispose();
				subscriptions = null;
				state.errorMessage = `Failed to initialize Work Center: ${toUserMessage(error)}`;
				initializationPromise = null;
				commit();
				throw error;
			});

			return initializationPromise;
		},
		dispose() {
			generation += 1;
			subscriptions?.dispose();
			subscriptions = null;
			initializationPromise = null;
			state.initialized = false;
			state.model = emptyWorkCenterModel();
			state.cancelPendingByOperationId = {};
			state.errorMessage = null;
			commit();
		},
		applyUpdate,
		async cancel(operationId, childJobId) {
			// A title cancel is idempotent in the backend and its snapshot shows
			// the request at once, so only whole-operation cancels track pending.
			if (childJobId !== undefined) {
				await sendCancel(operationId, childJobId);
				return;
			}
			const started = generation;
			state.cancelPendingByOperationId = {
				...state.cancelPendingByOperationId,
				[operationId]: true,
			};
			commit();
			try {
				await sendCancel(operationId);
			} finally {
				if (started === generation) {
					const next = { ...state.cancelPendingByOperationId };
					delete next[operationId];
					state.cancelPendingByOperationId = next;
					commit();
				}
			}
		},
		async revealOutput(child) {
			if (!child.outputPath) return;
			try {
				await tauriClient.revealPath(child.outputPath);
			} catch (error) {
				state.errorMessage = `Failed to show the exported file: ${toUserMessage(error)}`;
				commit();
			}
		},
	};
}
