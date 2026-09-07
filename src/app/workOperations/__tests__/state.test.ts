import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriClient } from '../../../lib/tauri/client';
import type { OperationListSnapshot, OperationSnapshot } from '../../../types/workRuntime';
import { createWorkOperationsSession, type WorkOperationsSession } from '../runtime';

const settleRemoteSourceMock = vi.fn();

function createDeferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (error: Error) => void;
	const promise = new Promise<T>((resolvePromise, rejectPromise) => {
		resolve = resolvePromise;
		reject = rejectPromise;
	});
	return { promise, resolve, reject };
}

function completedMergeOperation(operationId: string): OperationSnapshot {
	return {
		operationId,
		sequence: 1,
		revision: 1,
		createdRevision: 1,
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
	let session: WorkOperationsSession;

	beforeEach(() => {
		settleRemoteSourceMock.mockReset();
		settleRemoteSourceMock.mockResolvedValue(undefined);
		session = createWorkOperationsSession(() => undefined, {
			remoteSource: { settleTerminalWork: settleRemoteSourceMock },
		});
	});

	afterEach(() => {
		vi.restoreAllMocks();
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = undefined;
		session.dispose();
	});

	it('disposes registered listeners when initial operation listing fails', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		const snapshotUnlisten = vi.fn();
		const listUnlisten = vi.fn();
		vi.spyOn(tauriClient, 'listen')
			.mockResolvedValueOnce(snapshotUnlisten)
			.mockResolvedValueOnce(listUnlisten);
		vi.spyOn(tauriClient, 'listWorkOperations').mockRejectedValueOnce(new Error('list failed'));

		await expect(session.initialize()).rejects.toThrow('list failed');

		expect(snapshotUnlisten).toHaveBeenCalledTimes(1);
		expect(listUnlisten).toHaveBeenCalledTimes(1);
	});

	it('keeps event state when a delayed initial listing arrives', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		const list = createDeferred<OperationListSnapshot>();
		vi.spyOn(tauriClient, 'listen').mockResolvedValue(() => undefined);
		const listCall = vi.spyOn(tauriClient, 'listWorkOperations').mockReturnValue(list.promise);
		const initialize = session.initialize();
		await vi.waitFor(() => expect(listCall).toHaveBeenCalled());
		const completed = completedMergeOperation('op-finished');
		session.applyOperationSnapshot(completed);
		list.resolve({ membershipRevision: 0, operations: [] });
		await initialize;
		expect(session.view().operations).toEqual([completed]);
	});

	it('ignores an initial-list rejection after the session is disposed', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		const list = createDeferred<OperationListSnapshot>();
		vi.spyOn(tauriClient, 'listen').mockResolvedValue(() => undefined);
		const listCall = vi.spyOn(tauriClient, 'listWorkOperations').mockReturnValue(list.promise);
		const initializing = session.initialize();
		await vi.waitFor(() => expect(listCall).toHaveBeenCalled());
		session.dispose();
		list.reject(new Error('late list failure'));
		await expect(initializing).resolves.toBeUndefined();
		expect(session.view()).toMatchObject({
			initialized: false,
			operations: [],
			errorMessage: null,
		});
	});

	it.each([false, true])(
		'ignores delayed cancel response after terminal event (disposed=%s)',
		async (disposed) => {
			const response = createDeferred<OperationSnapshot>();
			vi.spyOn(tauriClient, 'cancelWorkOperation').mockReturnValue(response.promise);
			const completed = { ...completedMergeOperation('op-cancel'), revision: 3 };
			const cancel = session.cancel(completed.operationId);
			session.applyOperationSnapshot(completed);
			if (disposed) session.dispose();
			response.resolve({ ...completed, revision: 2, status: 'cancelling' });
			await cancel;
			expect(session.view().operations).toEqual(disposed ? [] : [completed]);
		},
	);

	it('purges completed merge operation source ids even when the merge child has no input id', async () => {
		session.applyOperationSnapshot(completedMergeOperation('op-merge-purge'));
		await Promise.resolve();

		expect(settleRemoteSourceMock).toHaveBeenCalledWith({
			inputIds: ['input-1', 'input-2'],
			completedInputIds: ['input-1', 'input-2'],
		});
	});

	it('claims terminal operation purge before awaiting so duplicate terminal snapshots do not race', async () => {
		let resolvePurge!: () => void;
		settleRemoteSourceMock.mockReturnValueOnce(
			new Promise<void>((resolve) => {
				resolvePurge = resolve;
			}),
		);
		const snapshot = completedMergeOperation('op-merge-race');

		session.applyOperationSnapshot(snapshot);
		session.applyOperationSnapshot(snapshot);
		await Promise.resolve();

		expect(settleRemoteSourceMock).toHaveBeenCalledTimes(1);
		resolvePurge();
	});

	it('surfaces source-open rejection without leaving an unhandled promise', async () => {
		vi.spyOn(tauriClient, 'openPath').mockRejectedValueOnce('source application unavailable');

		await expect(session.openSource({ sourcePath: '/tmp/book.m4b' })).resolves.toBeUndefined();

		expect(session.view().errorMessage).toBe(
			'Failed to open source file: source application unavailable',
		);
	});

	it('does not mark initialized or retain listeners when disposed mid-initialization', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		const snapshotUnlisten = vi.fn();
		const listUnlisten = vi.fn();
		const listDeferred = createDeferred<{ membershipRevision: number; operations: never[] }>();
		vi.spyOn(tauriClient, 'listen')
			.mockResolvedValueOnce(snapshotUnlisten)
			.mockResolvedValueOnce(listUnlisten);
		vi.spyOn(tauriClient, 'listWorkOperations').mockReturnValueOnce(
			listDeferred.promise as ReturnType<typeof tauriClient.listWorkOperations>,
		);

		const initPromise = session.initialize();
		await new Promise((resolve) => setTimeout(resolve, 0));
		session.dispose();
		listDeferred.resolve({ membershipRevision: 0, operations: [] });
		await initPromise.catch(() => {});

		expect(snapshotUnlisten).toHaveBeenCalledTimes(1);
		expect(listUnlisten).toHaveBeenCalledTimes(1);
		expect(session.view().initialized).toBe(false);
	});

	it('does not share operation snapshots across sessions', () => {
		const other = createWorkOperationsSession(() => undefined, {
			remoteSource: { settleTerminalWork: vi.fn(async () => undefined) },
		});
		session.applyOperationSnapshot(completedMergeOperation('op-isolation'));
		expect(session.view().operations).toHaveLength(1);
		expect(other.view().operations).toEqual([]);
		other.dispose();
	});
});
