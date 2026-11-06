import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriClient } from '../../../lib/tauri/client';
import type {
	OperationSnapshot,
	WorkOperationsSnapshot,
	WorkOperationsUpdate,
} from '../../../types/workRuntime';
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
		kind: 'processingBatch',
		status: 'completed',
		title: 'Tidy First',
		startedAtMs: 2,
		finishedAtMs: 3,
		cancellable: false,
		cancelRequested: false,
		sourceInputIds: ['input-1', 'input-2'],
		progress: {
			stage: 'complete',
			percentage: 100,
			message: 'Complete.',
			etaSeconds: undefined,
		},
		children: [
			{
				childJobId: `${operationId}-child`,
				operationId,
				label: 'Merge output',
				status: 'completed',
				progress: {
					stage: 'complete',
					percentage: 100,
					message: 'Complete.',
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
				warnings: [],
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
		logTail: [],
	};
}

type UpdateHandler = (event: { payload: WorkOperationsUpdate }) => void;

function captureUpdateHandlers(): UpdateHandler[] {
	const handlers: UpdateHandler[] = [];
	vi.spyOn(tauriClient, 'listen').mockImplementation(async (_event, handler: UpdateHandler) => {
		handlers.push(handler);
		return () => undefined;
	});
	return handlers;
}

function deliver(handler: UpdateHandler | undefined, operation: OperationSnapshot): void {
	handler?.({ payload: { revision: 1, order: [operation.operationId], changed: operation } });
}

function enableTauriRuntime(): void {
	(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
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
		const updateUnlisten = vi.fn();
		vi.spyOn(tauriClient, 'listen').mockResolvedValueOnce(updateUnlisten);
		vi.spyOn(tauriClient, 'listWorkOperations').mockRejectedValueOnce(new Error('list failed'));

		await expect(session.initialize()).rejects.toThrow('list failed');

		expect(updateUnlisten).toHaveBeenCalledTimes(1);
	});

	it('keeps event state when a delayed initial listing arrives', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		const list = createDeferred<WorkOperationsSnapshot>();
		const handlers = captureUpdateHandlers();
		const listCall = vi.spyOn(tauriClient, 'listWorkOperations').mockReturnValue(list.promise);
		const initialize = session.initialize();
		await vi.waitFor(() => expect(listCall).toHaveBeenCalled());
		const completed = completedExportOperation('op-finished');
		deliver(handlers[0], completed);
		list.resolve({ revision: 0, order: [], operations: [] });
		await initialize;
		expect(session.view().operations).toEqual([completed]);
	});

	it('ignores an initial-list rejection after the session is disposed', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		const list = createDeferred<WorkOperationsSnapshot>();
		vi.spyOn(tauriClient, 'listen').mockResolvedValue(() => undefined);
		const listCall = vi.spyOn(tauriClient, 'listWorkOperations').mockReturnValue(list.promise);
		const initializing = session.initialize();
		await vi.waitFor(() => expect(listCall).toHaveBeenCalled());
		session.dispose();
		list.reject(new Error('late list failure'));
		await expect(initializing).resolves.toBeUndefined();
		expect(session.view()).toMatchObject({
			operations: [],
			errorMessage: null,
		});
	});

	it.each([false, true])(
		'ignores delayed cancel response after terminal event (disposed=%s)',
		async (disposed) => {
			enableTauriRuntime();
			const handlers = captureUpdateHandlers();
			vi.spyOn(tauriClient, 'listWorkOperations').mockResolvedValue({
				revision: 0,
				order: [],
				operations: [],
			});
			await session.initialize();
			const response = createDeferred<OperationSnapshot>();
			vi.spyOn(tauriClient, 'cancelWorkOperation').mockReturnValue(response.promise);
			const completed = { ...completedExportOperation('op-cancel'), revision: 3 };
			const cancel = session.cancel(completed.operationId);
			deliver(handlers[0], completed);
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

	it('applies no list or retained listeners when disposed mid-initialization', async () => {
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
		const updateUnlisten = vi.fn();
		const listDeferred = createDeferred<WorkOperationsSnapshot>();
		vi.spyOn(tauriClient, 'listen').mockResolvedValueOnce(updateUnlisten);
		vi.spyOn(tauriClient, 'listWorkOperations').mockReturnValueOnce(listDeferred.promise);

		const initPromise = session.initialize();
		await new Promise((resolve) => setTimeout(resolve, 0));
		session.dispose();
		listDeferred.resolve({ revision: 0, order: [], operations: [] });
		await initPromise.catch(() => {});

		expect(updateUnlisten).toHaveBeenCalledTimes(1);
		expect(session.view().operations).toEqual([]);
	});

	it('does not share operation snapshots across sessions', async () => {
		enableTauriRuntime();
		const handlers = captureUpdateHandlers();
		vi.spyOn(tauriClient, 'listWorkOperations').mockResolvedValue({
			revision: 0,
			order: [],
			operations: [],
		});
		const other = createWorkOperationsSession(() => undefined);
		await session.initialize();
		await other.initialize();
		const isolated = completedExportOperation('op-isolation');
		deliver(handlers[0], isolated);
		expect(session.view().operations).toHaveLength(1);
		expect(other.view().operations).toEqual([]);
		other.dispose();
	});
});
