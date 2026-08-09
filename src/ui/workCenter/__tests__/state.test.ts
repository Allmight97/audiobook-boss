import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriClient } from '../../../lib/tauri/client';
import type { OperationSnapshot } from '../../../types/workRuntime';

const { purgeMock, releaseMock } = vi.hoisted(() => ({
	purgeMock: vi.fn(),
	releaseMock: vi.fn(),
}));

vi.mock('../../remoteSource/sessionAssets.svelte', () => ({
	purgeRemoteSourceSessionsForInputIds: purgeMock,
	releaseRemoteSourceSessionRetainers: releaseMock,
}));

import { applyOperationSnapshot, disposeWorkCenter, initializeWorkCenter } from '../state.svelte';
import { workCenterState } from '../state.svelte';

function createDeferred<T>() {
	let resolve!: (value: T) => void;
	const promise = new Promise<T>((resolvePromise) => {
		resolve = resolvePromise;
	});
	return { promise, resolve };
}

function completedMergeOperation(operationId: string): OperationSnapshot {
	return {
		operationId,
		sequence: 1,
		kind: 'processingMerge',
		status: 'completed',
		title: 'Merge encode',
		createdAtMs: 1,
		startedAtMs: 2,
		finishedAtMs: 3,
		cancellable: false,
		cancelRequested: false,
		lanes: ['analysis', 'encodeCpu', 'outputCommit'],
		sourceInputIds: ['input-1', 'input-2'],
		progress: {
			stage: 'complete',
			percentage: 100,
			message: 'Complete.',
			currentItemIndex: undefined,
			totalItems: 1,
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			etaSeconds: undefined,
		},
		children: [
			{
				childJobId: `${operationId}-child`,
				operationId,
				label: 'Merge output',
				status: 'completed',
				lane: 'encodeCpu',
				progress: {
					stage: 'complete',
					percentage: 100,
					message: 'Complete.',
					currentItemIndex: undefined,
					totalItems: 1,
					bytesDownloaded: undefined,
					bytesTotal: undefined,
					etaSeconds: undefined,
				},
				sourcePath: undefined,
				inputIndex: undefined,
				inputId: undefined,
				jobId: 'job-1',
				cancellable: false,
				cancelRequested: false,
				message: 'Complete.',
			},
		],
		terminalSummary: {
			total: 1,
			succeeded: 1,
			skipped: 0,
			cancelled: 0,
			failed: 0,
			message: 'Completed 1/1.',
		},
		warnings: [],
		errors: [],
		logTail: [],
	};
}

describe('Work Center state', () => {
	beforeEach(() => {
		purgeMock.mockReset();
		purgeMock.mockResolvedValue(undefined);
		releaseMock.mockReset();
		releaseMock.mockReturnValue([]);
		disposeWorkCenter();
	});

	afterEach(() => {
		vi.restoreAllMocks();
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = undefined;
		disposeWorkCenter();
	});

	it('disposes registered listeners when initial operation listing fails', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		const snapshotUnlisten = vi.fn();
		const listUnlisten = vi.fn();
		vi.spyOn(tauriClient, 'listen')
			.mockResolvedValueOnce(snapshotUnlisten)
			.mockResolvedValueOnce(listUnlisten);
		vi.spyOn(tauriClient, 'listWorkOperations').mockRejectedValueOnce(new Error('list failed'));

		await expect(initializeWorkCenter()).rejects.toThrow('list failed');

		expect(snapshotUnlisten).toHaveBeenCalledTimes(1);
		expect(listUnlisten).toHaveBeenCalledTimes(1);
	});

	it('purges completed merge operation source ids even when the merge child has no input id', async () => {
		applyOperationSnapshot(completedMergeOperation('op-merge-purge'));
		await Promise.resolve();

		expect(releaseMock).toHaveBeenCalledWith(['input-1', 'input-2']);
		expect(purgeMock).toHaveBeenCalledWith(['input-1', 'input-2']);
	});

	it('claims terminal operation purge before awaiting so duplicate terminal snapshots do not race', async () => {
		let resolvePurge!: () => void;
		purgeMock.mockReturnValueOnce(
			new Promise<void>((resolve) => {
				resolvePurge = resolve;
			}),
		);
		const snapshot = completedMergeOperation('op-merge-race');

		applyOperationSnapshot(snapshot);
		applyOperationSnapshot(snapshot);
		await Promise.resolve();

		expect(releaseMock).toHaveBeenCalledTimes(1);
		expect(purgeMock).toHaveBeenCalledTimes(1);
		resolvePurge();
	});

	it('does not mark initialized or retain listeners when disposed mid-initialization', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		const snapshotUnlisten = vi.fn();
		const listUnlisten = vi.fn();
		const listDeferred = createDeferred<{ operations: never[] }>();
		vi.spyOn(tauriClient, 'listen')
			.mockResolvedValueOnce(snapshotUnlisten)
			.mockResolvedValueOnce(listUnlisten);
		vi.spyOn(tauriClient, 'listWorkOperations').mockReturnValueOnce(
			listDeferred.promise as ReturnType<typeof tauriClient.listWorkOperations>,
		);

		const initPromise = initializeWorkCenter();
		// Flush microtasks so both listeners register and init parks on listWorkOperations.
		await new Promise((resolve) => setTimeout(resolve, 0));
		disposeWorkCenter();
		listDeferred.resolve({ operations: [] });
		await initPromise.catch(() => {});

		// Dispose won the race: listeners torn down, state not marked initialized.
		expect(snapshotUnlisten).toHaveBeenCalledTimes(1);
		expect(listUnlisten).toHaveBeenCalledTimes(1);
		expect(workCenterState.initialized).toBe(false);
	});
});
