import { describe, expect, it, vi } from 'vitest';

import { Effect, runAppEffect } from '../../../lib/effect/appEffect';
import type { FileListInfo } from '../../../types/audio';
import type { MetadataSaveBatchResult, MetadataSaveResultEntry } from '../../../types/metadata';
import type { MetadataIntentPatch } from '../../../types/metadataIntent';
import {
	MetadataSaveWorkflowFailed,
	makeMetadataSaveWorkflowServicesLayer,
	metadataSaveWorkflowExecution,
	runMetadataSaveWorkflow,
	type MetadataSaveWorkflowServices,
} from '../metadataSaveWorkflow';

type PendingEntry = [string, MetadataIntentPatch];
type SaveBatchItems = Parameters<MetadataSaveWorkflowServices['saveMetadataBatch']>[0];

function titlePatch(title: string): MetadataIntentPatch {
	return { title: { op: 'set', value: title } };
}

function fileListFromEntries(entries: Array<{ path: string; isValid: boolean }>): FileListInfo {
	return {
		files: entries,
		selectedDecoders: entries.map(() => null),
		totalDuration: 0,
		totalSize: 0,
		validCount: entries.filter((entry) => entry.isValid).length,
		invalidCount: entries.filter((entry) => !entry.isValid).length,
	} as FileListInfo;
}

function defaultFileList(): FileListInfo {
	return fileListFromEntries([
		{ path: '/books/a.m4b', isValid: true },
		{ path: '/books/b.m4b', isValid: true },
	]);
}

function entry(
	filePath: string,
	status: MetadataSaveResultEntry['status'],
	inputIndex: number,
): MetadataSaveResultEntry {
	return {
		inputIndex,
		filePath,
		status,
		message: status === 'success' ? 'saved' : `${status} metadata save`,
	};
}

function resultFor(entries: MetadataSaveResultEntry[]): MetadataSaveBatchResult {
	return {
		summary: {
			total: entries.length,
			succeeded: entries.filter((candidate) => candidate.status === 'success').length,
			skipped: 0,
			cancelled: entries.filter((candidate) => candidate.status === 'cancelled').length,
			failed: entries.filter((candidate) => candidate.status === 'failed').length,
		},
		results: entries,
	};
}

function successResultForItems(items: SaveBatchItems): MetadataSaveBatchResult {
	return resultFor(items.map((item, index) => entry(item.filePath, 'success', index)));
}

function makeHarness(options?: {
	fileList?: FileListInfo | null;
	isProcessing?: boolean;
	saveInProgress?: boolean;
	pendingEntries?: PendingEntry[];
	persistDrafts?: () => Promise<boolean>;
	saveMetadataBatch?: (items: SaveBatchItems) => Promise<MetadataSaveBatchResult>;
}) {
	let saveInProgress = options?.saveInProgress ?? false;
	const fileList = options?.fileList === undefined ? defaultFileList() : options.fileList;
	const pendingEntries = options?.pendingEntries ?? [
		['/books/a.m4b', titlePatch('A')],
		['/books/b.m4b', titlePatch('B')],
	];

	const getCurrentFileList = vi.fn(() => fileList);
	const initStatusPanel = vi.fn(() => undefined);
	const isStatusPanelProcessing = vi.fn(() => options?.isProcessing ?? false);
	const pushStatusPanelTransientStatus = vi.fn();
	const isMetadataSaveInProgress = vi.fn(() => saveInProgress);
	const setMetadataSaveInProgress = vi.fn((isInProgress: boolean) => {
		saveInProgress = isInProgress;
	});
	const persistPendingMetadataDraftsForCurrentSelection = vi.fn(
		options?.persistDrafts ?? (async () => true),
	);
	const getPendingMetadataIntentEntries = vi.fn(() => pendingEntries);
	const saveMetadataBatch = vi.fn(
		options?.saveMetadataBatch ?? (async (items: SaveBatchItems) => successResultForItems(items)),
	);
	const clearPendingMetadataForFile = vi.fn();
	const resetDirtyState = vi.fn();
	const beginMetadataSaveInStatusPanel = vi.fn(async () => undefined);
	const completeMetadataSaveInStatusPanel = vi.fn();
	const failMetadataSaveInStatusPanel = vi.fn();
	const consoleLog = vi.fn();
	const consoleError = vi.fn();

	const services = {
		getCurrentFileList,
		initStatusPanel,
		isStatusPanelProcessing,
		pushStatusPanelTransientStatus,
		isMetadataSaveInProgress,
		setMetadataSaveInProgress,
		persistPendingMetadataDraftsForCurrentSelection,
		getPendingMetadataIntentEntries,
		saveMetadataBatch,
		clearPendingMetadataForFile,
		resetDirtyState,
		beginMetadataSaveInStatusPanel,
		completeMetadataSaveInStatusPanel,
		failMetadataSaveInStatusPanel,
		console: {
			log: consoleLog,
			error: consoleError,
		},
	} as unknown as MetadataSaveWorkflowServices;

	return {
		layer: makeMetadataSaveWorkflowServicesLayer(services),
		mocks: {
			getCurrentFileList,
			initStatusPanel,
			isStatusPanelProcessing,
			pushStatusPanelTransientStatus,
			isMetadataSaveInProgress,
			setMetadataSaveInProgress,
			persistPendingMetadataDraftsForCurrentSelection,
			getPendingMetadataIntentEntries,
			saveMetadataBatch,
			clearPendingMetadataForFile,
			resetDirtyState,
			beginMetadataSaveInStatusPanel,
			completeMetadataSaveInStatusPanel,
			failMetadataSaveInStatusPanel,
			consoleLog,
			consoleError,
		},
		getSaveInProgress: () => saveInProgress,
	};
}

describe('MetadataSaveWorkflow', () => {
	it('returns without status mutation when no files are loaded', async () => {
		const harness = makeHarness({ fileList: null });

		await runMetadataSaveWorkflow(harness.layer);

		expect(harness.mocks.consoleLog).toHaveBeenCalledWith('No files loaded - nothing to save');
		expect(harness.mocks.initStatusPanel).not.toHaveBeenCalled();
		expect(harness.mocks.pushStatusPanelTransientStatus).not.toHaveBeenCalled();
		expect(harness.mocks.setMetadataSaveInProgress).not.toHaveBeenCalled();
		expect(harness.mocks.saveMetadataBatch).not.toHaveBeenCalled();
	});

	it('short-circuits while status panel processing is active', async () => {
		const harness = makeHarness({ isProcessing: true });

		await runMetadataSaveWorkflow(harness.layer);

		expect(harness.mocks.initStatusPanel).toHaveBeenCalledTimes(1);
		expect(harness.mocks.consoleLog).toHaveBeenCalledWith(
			'Processing in progress - cannot save metadata now',
		);
		expect(harness.mocks.pushStatusPanelTransientStatus).not.toHaveBeenCalled();
		expect(harness.mocks.persistPendingMetadataDraftsForCurrentSelection).not.toHaveBeenCalled();
		expect(harness.mocks.saveMetadataBatch).not.toHaveBeenCalled();
	});

	it('short-circuits while another metadata save is in progress', async () => {
		const harness = makeHarness({ saveInProgress: true });

		await runMetadataSaveWorkflow(harness.layer);

		expect(harness.mocks.pushStatusPanelTransientStatus).toHaveBeenNthCalledWith(
			1,
			'Preparing metadata save...',
			{ ttlMs: 1_000 },
		);
		expect(harness.mocks.pushStatusPanelTransientStatus).toHaveBeenNthCalledWith(
			2,
			'Save already in progress...',
			{ ttlMs: 1_500 },
		);
		expect(harness.mocks.setMetadataSaveInProgress).not.toHaveBeenCalled();
		expect(harness.mocks.saveMetadataBatch).not.toHaveBeenCalled();
	});

	it('stops when draft persistence reports validation failure', async () => {
		const harness = makeHarness({ persistDrafts: async () => false });

		await runMetadataSaveWorkflow(harness.layer);

		expect(harness.mocks.persistPendingMetadataDraftsForCurrentSelection).toHaveBeenCalledWith({
			showStatus: false,
		});
		expect(harness.mocks.pushStatusPanelTransientStatus).toHaveBeenCalledWith(
			'Fix metadata validation errors before saving.',
			{ ttlMs: 3_000 },
		);
		expect(harness.mocks.saveMetadataBatch).not.toHaveBeenCalled();
		expect(harness.mocks.resetDirtyState).not.toHaveBeenCalled();
		expect(harness.getSaveInProgress()).toBe(false);
	});

	it('reports no-op when there are no pending metadata changes for loaded valid files', async () => {
		const harness = makeHarness({
			pendingEntries: [['/books/missing.m4b', titlePatch('Missing')]],
		});

		await runMetadataSaveWorkflow(harness.layer);

		expect(harness.mocks.pushStatusPanelTransientStatus).toHaveBeenCalledWith(
			'No pending metadata changes',
			{ ttlMs: 2_000 },
		);
		expect(harness.mocks.beginMetadataSaveInStatusPanel).not.toHaveBeenCalled();
		expect(harness.mocks.saveMetadataBatch).not.toHaveBeenCalled();
		expect(harness.mocks.resetDirtyState).not.toHaveBeenCalled();
		expect(harness.getSaveInProgress()).toBe(false);
	});

	it('saves pending changes, clears successful entries, and resets dirty state', async () => {
		const harness = makeHarness({
			pendingEntries: [
				['/books/a.m4b', titlePatch('A')],
				['/books/b.m4b', titlePatch('B')],
				['/books/missing.m4b', titlePatch('Missing')],
			],
		});

		await runMetadataSaveWorkflow(harness.layer);

		expect(harness.mocks.beginMetadataSaveInStatusPanel).toHaveBeenCalledTimes(1);
		expect(harness.mocks.saveMetadataBatch).toHaveBeenCalledWith([
			{ filePath: '/books/a.m4b', metadataPatch: titlePatch('A') },
			{ filePath: '/books/b.m4b', metadataPatch: titlePatch('B') },
		]);
		expect(harness.mocks.clearPendingMetadataForFile).toHaveBeenCalledTimes(2);
		expect(harness.mocks.clearPendingMetadataForFile).toHaveBeenCalledWith('/books/a.m4b');
		expect(harness.mocks.clearPendingMetadataForFile).toHaveBeenCalledWith('/books/b.m4b');
		expect(harness.mocks.resetDirtyState).toHaveBeenCalledTimes(1);
		expect(harness.mocks.completeMetadataSaveInStatusPanel).toHaveBeenCalledTimes(1);
		expect(harness.getSaveInProgress()).toBe(false);
	});

	it('leaves failed and cancelled entries pending while reporting failed entries', async () => {
		const fileList = fileListFromEntries([
			{ path: '/books/a.m4b', isValid: true },
			{ path: '/books/b.m4b', isValid: true },
			{ path: '/books/c.m4b', isValid: true },
		]);
		const result = resultFor([
			entry('/books/a.m4b', 'success', 0),
			entry('/books/b.m4b', 'failed', 1),
			entry('/books/c.m4b', 'cancelled', 2),
		]);
		const harness = makeHarness({
			fileList,
			pendingEntries: [
				['/books/a.m4b', titlePatch('A')],
				['/books/b.m4b', titlePatch('B')],
				['/books/c.m4b', titlePatch('C')],
			],
			saveMetadataBatch: async () => result,
		});

		await runMetadataSaveWorkflow(harness.layer);

		expect(harness.mocks.clearPendingMetadataForFile).toHaveBeenCalledTimes(1);
		expect(harness.mocks.clearPendingMetadataForFile).toHaveBeenCalledWith('/books/a.m4b');
		expect(harness.mocks.consoleError).toHaveBeenCalledWith(
			'Failed metadata save for /books/b.m4b:',
			'failed metadata save',
		);
		expect(harness.mocks.resetDirtyState).toHaveBeenCalledTimes(1);
		expect(harness.mocks.completeMetadataSaveInStatusPanel).toHaveBeenCalledWith(result);
		expect(harness.getSaveInProgress()).toBe(false);
	});

	it('reports thrown save failures and clears in-progress state', async () => {
		const cause = new Error('disk write failed');
		const harness = makeHarness({
			saveMetadataBatch: async () => {
				throw cause;
			},
		});

		await runMetadataSaveWorkflow(harness.layer);

		expect(harness.mocks.failMetadataSaveInStatusPanel).toHaveBeenCalledWith(
			'Save failed - see console',
		);
		expect(harness.mocks.pushStatusPanelTransientStatus).toHaveBeenCalledWith(
			'Save failed - see console',
			{ ttlMs: 3_000 },
		);
		expect(harness.mocks.completeMetadataSaveInStatusPanel).not.toHaveBeenCalled();
		expect(harness.getSaveInProgress()).toBe(false);
	});

	it('exposes typed workflow errors for rejected infrastructure calls', async () => {
		const cause = new Error('draft persistence failed');
		const harness = makeHarness({
			persistDrafts: async () => {
				throw cause;
			},
		});

		const error = await runAppEffect(
			Effect.flip(metadataSaveWorkflowExecution().pipe(Effect.provide(harness.layer))),
		);

		expect(error).toBeInstanceOf(MetadataSaveWorkflowFailed);
		expect(error.message).toBe('Failed to persist pending metadata drafts.');
		expect(error.cause).toBe(cause);
		expect(harness.getSaveInProgress()).toBe(false);
	});
});
