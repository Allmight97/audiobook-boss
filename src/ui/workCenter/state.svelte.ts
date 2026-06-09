import { tauriClient } from '../../lib/tauri/client';
import { EVENTS } from '../../types/events';
import type { OperationId, OperationSnapshot } from '../../types/workRuntime';
import {
	purgeRemoteSourceSessionsForInputIds,
	releaseRemoteSourceSessionRetainers,
} from '../remoteSource/sessionAssets.svelte';
import {
	isTerminalOperationStatus,
	replaceOperations,
	upsertOperation,
	type WorkCenterModel,
} from './model';

type Unlisten = () => void;

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

let initializationPromise: Promise<void> | null = null;
let unlisteners: Unlisten[] = [];
const purgedOperationIds = new Set<string>();

export function initializeWorkCenter(): Promise<void> {
	if (initializationPromise) return initializationPromise;
	if (!isTauriRuntimeAvailable()) {
		workCenterState.initialized = true;
		workCenterState.errorMessage = null;
		return Promise.resolve();
	}

	initializationPromise = (async () => {
		const nextUnlisteners: Unlisten[] = [];
		try {
			nextUnlisteners.push(
				await tauriClient.listen(EVENTS.WORK_OPERATION_SNAPSHOT, ({ payload }) => {
					applyOperationSnapshot(payload.snapshot);
				}),
			);
			nextUnlisteners.push(
				await tauriClient.listen(EVENTS.WORK_OPERATION_LIST_SNAPSHOT, ({ payload }) => {
					const model = replaceOperations(workCenterState, { operations: payload.operations });
					workCenterState.operations = model.operations;
					for (const operation of workCenterState.operations) {
						void purgeRemoteSessionsForTerminalOperation(operation);
					}
				}),
			);
			unlisteners = nextUnlisteners;

			const list = await tauriClient.listWorkOperations();
			const model = replaceOperations(workCenterState, list);
			workCenterState.operations = model.operations;
			workCenterState.initialized = true;
			workCenterState.errorMessage = null;
		} catch (error) {
			disposeUnlisteners(nextUnlisteners);
			if (unlisteners === nextUnlisteners) {
				unlisteners = [];
			}
			throw error;
		}
	})().catch((error) => {
		workCenterState.errorMessage = `Failed to initialize Work Center: ${String(error)}`;
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
	disposeUnlisteners(unlisteners);
	unlisteners = [];
	initializationPromise = null;
	workCenterState.initialized = false;
}

export function applyOperationSnapshot(snapshot: OperationSnapshot): void {
	const model = upsertOperation(workCenterState, snapshot);
	workCenterState.operations = model.operations;
	void purgeRemoteSessionsForTerminalOperation(snapshot);
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
		workCenterState.errorMessage = `Failed to cancel operation: ${String(error)}`;
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
	purgedOperationIds.add(operation.operationId);

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

function disposeUnlisteners(listeners: Unlisten[]): void {
	for (const unlisten of listeners.splice(0)) {
		unlisten();
	}
}
