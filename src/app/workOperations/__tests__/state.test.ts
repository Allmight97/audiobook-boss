import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriClient } from '../../../lib/tauri/client';
import { publishMockMetadataSave } from '../../../test/setup';
import type { OperationListSnapshot, OperationSnapshot } from '../../../types/workRuntime';
import { createWorkOperationsSession, type WorkOperationsSession } from '../runtime';

function createDeferred<T>() {
	let resolve!: (value: T) => void;
	let reject!: (error: Error) => void;
	const promise = new Promise<T>((resolvePromise, rejectPromise) => {
		resolve = resolvePromise;
		reject = rejectPromise;
	});
	return { promise, resolve, reject };
}

function completedExportOperation(operationId: string): OperationSnapshot {
	return {
		operationId,
		sequence: 1,
		revision: 1,
		createdRevision: 1,
		kind: 'processingBatch',
		status: 'completed',
		title: 'Tidy First',
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
				inputId: 'input-1',
				sourceInputIds: ['input-1', 'input-2'],
				jobId: 'job-1',
				cancellable: false,
				cancelRequested: false,
				message: 'Complete.',
				supplementalWarning: undefined,
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
		errors: [],
		logTail: [],
	};
}

describe('Work Center state', () => {
	let session: WorkOperationsSession;

	beforeEach(() => {
		session = createWorkOperationsSession(() => undefined);
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

	it('retains successive metadata operations and their terminal updates through the Tauri mock', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		await session.initialize();
		for (const filePath of ['/books/alpha.m4b', '/books/beta.m4b']) {
			publishMockMetadataSave([filePath]);
		}
		const operations = session.view().operations;
		expect(operations).toHaveLength(2);
		expect(operations.map(({ status }) => status)).toEqual(['completed', 'completed']);
		const listed = await tauriClient.listWorkOperations();
		expect(listed.operations).toEqual(operations);
		expect(await tauriClient.listWorkOperations()).toEqual(listed);
	});

	it('keeps event state when a delayed initial listing arrives', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		const list = createDeferred<OperationListSnapshot>();
		vi.spyOn(tauriClient, 'listen').mockResolvedValue(() => undefined);
		const listCall = vi.spyOn(tauriClient, 'listWorkOperations').mockReturnValue(list.promise);
		const initialize = session.initialize();
		await vi.waitFor(() => expect(listCall).toHaveBeenCalled());
		const completed = completedExportOperation('op-finished');
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
			const completed = { ...completedExportOperation('op-cancel'), revision: 3 };
			const cancel = session.cancel(completed.operationId);
			session.applyOperationSnapshot(completed);
			if (disposed) session.dispose();
			response.resolve({ ...completed, revision: 2, status: 'cancelling' });
			await cancel;
			expect(session.view().operations).toEqual(disposed ? [] : [completed]);
		},
	);

	it('surfaces reveal rejection without leaving an unhandled promise', async () => {
		vi.spyOn(tauriClient, 'revealPath').mockRejectedValueOnce('file manager unavailable');

		await expect(session.revealOutput({ outputPath: '/tmp/book.m4b' })).resolves.toBeUndefined();

		expect(session.view().errorMessage).toBe(
			'Failed to show the exported file: file manager unavailable',
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
		const other = createWorkOperationsSession(() => undefined);
		session.applyOperationSnapshot(completedExportOperation('op-isolation'));
		expect(session.view().operations).toHaveLength(1);
		expect(other.view().operations).toEqual([]);
		other.dispose();
	});
});
