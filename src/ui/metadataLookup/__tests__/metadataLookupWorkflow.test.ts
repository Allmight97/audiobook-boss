import { describe, expect, it, vi } from 'vitest';

import { Effect, runAppEffect } from '../../../lib/effect/appEffect';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import type { AudiobookMetadata, OnlineMetadataResult } from '../../../types/metadata';
import {
	makeMetadataLookupWorkflowServicesLayer,
	MetadataLookupWorkflowFailed,
	metadataLookupWorkflowExecution,
	runMetadataLookupWorkflow,
	type MetadataLookupWorkflowServices,
} from '../metadataLookupWorkflow';
import type {
	MetadataLookupQueueItem,
	MetadataLookupQueueState,
	MetadataLookupState,
} from '../state.svelte';

function audioFile(path: string, isValid = true): AudioFile {
	return {
		path,
		isValid,
		duration: 1,
		size: 100,
		format: 'm4b',
	} as AudioFile;
}

function fileList(files: AudioFile[]): FileListInfo {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length,
		totalSize: files.length * 100,
		validCount: files.filter((file) => file.isValid).length,
		invalidCount: files.filter((file) => !file.isValid).length,
	} as FileListInfo;
}

function lookupResult(overrides: Partial<OnlineMetadataResult> = {}): OnlineMetadataResult {
	return {
		source: 'audnexus',
		sourceId: 'audnexus:1',
		title: 'Lookup Title',
		authors: ['Author One'],
		narrators: ['Narrator One'],
		description: 'Description',
		publishedDate: '2020-07',
		durationSeconds: 3600,
		audibleOnly: false,
		...overrides,
	};
}

function defaultLookupState(overrides: Partial<MetadataLookupState> = {}): MetadataLookupState {
	return {
		isOpen: false,
		query: '',
		source: 'auto',
		applyMode: 'current',
		replaceCoverArt: false,
		statusMessage: '',
		statusVariant: 'info',
		queueContext: 'No files selected.',
		results: [],
		isQueueMode: false,
		skipEnabled: false,
		hasSearched: false,
		...overrides,
	};
}

function makeHarness(options?: {
	currentFileList?: FileListInfo | null;
	selectedIndices?: Set<number>;
	lookupState?: Partial<MetadataLookupState>;
	queueState?: Partial<MetadataLookupQueueState>;
	metadataByFile?: Array<[string, Partial<AudiobookMetadata>]>;
	readMetadataForm?: () => Partial<AudiobookMetadata>;
	searchOnlineMetadata?: MetadataLookupWorkflowServices['searchOnlineMetadata'];
	loadCoverArtFromUrl?: MetadataLookupWorkflowServices['loadCoverArtFromUrl'];
	selectFile?: MetadataLookupWorkflowServices['selectFile'];
}) {
	const files = [audioFile('/books/alpha.m4b'), audioFile('/books/beta.m4b')];
	const lookupState = defaultLookupState(options?.lookupState);
	const queueState: MetadataLookupQueueState = {
		queue: [],
		index: 0,
		...options?.queueState,
	};
	const metadataByFile = new Map<string, Partial<AudiobookMetadata>>(
		options?.metadataByFile ?? [
			['/books/alpha.m4b', { title: 'Alpha Existing', album: 'Alpha Existing' }],
			['/books/beta.m4b', { title: 'Beta Existing', album: 'Beta Existing', cover_art: [2] }],
		],
	);
	const currentFileList =
		options?.currentFileList === undefined ? fileList(files) : options.currentFileList;
	const selectedIndices = options?.selectedIndices ?? new Set<number>([0, 1]);

	const getLookupState = vi.fn(() => lookupState);
	const getQueueState = vi.fn(() => queueState);
	const setMetadataLookupQueue = vi.fn((queue: MetadataLookupQueueItem[]) => {
		queueState.queue = queue;
		queueState.index = 0;
	});
	const clearMetadataLookupQueue = vi.fn(() => {
		queueState.queue = [];
		queueState.index = 0;
	});
	const setMetadataLookupQueueIndex = vi.fn((index: number) => {
		queueState.index = index;
	});
	const getSelectedFileIndices = vi.fn(() => selectedIndices);
	const getCurrentFileList = vi.fn(() => currentFileList);
	const getMetadataForFile = vi.fn((filePath: string) => metadataByFile.get(filePath));
	const setMetadataForFile = vi.fn((filePath: string, metadata: Partial<AudiobookMetadata>) => {
		metadataByFile.set(filePath, metadata);
	});
	const selectFile = vi.fn(
		options?.selectFile ?? (async () => undefined),
	) as MetadataLookupWorkflowServices['selectFile'] & ReturnType<typeof vi.fn>;
	const applyMetadataToForm = vi.fn();
	const readMetadataForm = vi.fn(options?.readMetadataForm ?? (() => ({ title: 'Patched Title' })));
	const updateOutputPath = vi.fn();
	const updateEstimatedSize = vi.fn();
	const updateTagPreview = vi.fn();
	const clearCoverArt = vi.fn();
	const setCoverArt = vi.fn();
	const setCustomCoverArt = vi.fn();
	const searchOnlineMetadata = vi.fn(
		options?.searchOnlineMetadata ?? (async () => [lookupResult()]),
	);
	const loadCoverArtFromUrl = vi.fn(options?.loadCoverArtFromUrl ?? (async () => [9, 9, 9]));
	const focusElementById = vi.fn();
	const queueMicrotask = vi.fn((callback: () => void) => callback());
	const consoleError = vi.fn();
	const consoleWarn = vi.fn();

	const services = {
		getLookupState,
		getQueueState,
		setMetadataLookupQueue,
		clearMetadataLookupQueue,
		setMetadataLookupQueueIndex,
		getSelectedFileIndices,
		getCurrentFileList,
		getMetadataForFile,
		setMetadataForFile,
		selectFile,
		applyMetadataToForm,
		readMetadataForm,
		updateOutputPath,
		updateEstimatedSize,
		updateTagPreview,
		clearCoverArt,
		setCoverArt,
		setCustomCoverArt,
		searchOnlineMetadata,
		loadCoverArtFromUrl,
		focusElementById,
		queueMicrotask,
		console: {
			error: consoleError,
			warn: consoleWarn,
		},
	} satisfies MetadataLookupWorkflowServices;

	return {
		layer: makeMetadataLookupWorkflowServicesLayer(services),
		lookupState,
		queueState,
		metadataByFile,
		mocks: {
			setMetadataLookupQueue,
			clearMetadataLookupQueue,
			setMetadataLookupQueueIndex,
			setMetadataForFile,
			selectFile,
			applyMetadataToForm,
			readMetadataForm,
			updateOutputPath,
			updateEstimatedSize,
			updateTagPreview,
			clearCoverArt,
			setCoverArt,
			setCustomCoverArt,
			searchOnlineMetadata,
			loadCoverArtFromUrl,
			focusElementById,
			queueMicrotask,
			consoleError,
			consoleWarn,
		},
	};
}

describe('MetadataLookupWorkflow', () => {
	it('opens with an error when no selected files are valid', async () => {
		const harness = makeHarness({
			selectedIndices: new Set<number>([0]),
			currentFileList: fileList([audioFile('/books/invalid.m4b', false)]),
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'open' });

		expect(harness.queueState.queue).toEqual([]);
		expect(harness.lookupState.query).toBe('');
		expect(harness.lookupState.statusMessage).toBe('Select a valid file to search metadata.');
		expect(harness.lookupState.statusVariant).toBe('error');
		expect(harness.lookupState.isOpen).toBe(true);
	});

	it('opens a selected-file queue and derives the first query from stored metadata', async () => {
		const harness = makeHarness();

		await runMetadataLookupWorkflow(harness.layer, { type: 'open' });

		expect(harness.queueState.queue.map((item) => item.file.path)).toEqual([
			'/books/alpha.m4b',
			'/books/beta.m4b',
		]);
		expect(harness.lookupState.query).toBe('Alpha Existing');
		expect(harness.lookupState.queueContext).toBe('1 of 2 • alpha.m4b');
		expect(harness.lookupState.applyMode).toBe('queue');
		expect(harness.lookupState.skipEnabled).toBe(true);
	});

	it('rejects empty searches without calling the backend', async () => {
		const harness = makeHarness({ lookupState: { query: '   ' } });

		await runMetadataLookupWorkflow(harness.layer, { type: 'search' });

		expect(harness.mocks.searchOnlineMetadata).not.toHaveBeenCalled();
		expect(harness.lookupState.statusMessage).toBe('Enter a title, author, or ASIN to search.');
		expect(harness.lookupState.statusVariant).toBe('error');
	});

	it('stores search results and sends explicit source selection to Tauri', async () => {
		const result = lookupResult({ title: 'Found Title' });
		const harness = makeHarness({
			lookupState: { query: 'alpha', source: 'auto' },
			searchOnlineMetadata: async () => [result],
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'search' });

		expect(harness.mocks.searchOnlineMetadata).toHaveBeenCalledWith({
			query: 'alpha',
			sources: ['audnexus', 'openlibrary'],
			limit: 8,
		});
		expect(harness.lookupState.results).toEqual([result]);
		expect(harness.lookupState.hasSearched).toBe(true);
		expect(harness.lookupState.statusMessage).toBe('Found 1 results.');
	});

	it('surfaces search failures without treating them as no-result matches', async () => {
		const cause = new Error('all sources failed');
		const harness = makeHarness({
			lookupState: { query: 'alpha', results: [lookupResult()], hasSearched: true },
			searchOnlineMetadata: async () => {
				throw cause;
			},
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'search' });

		expect(harness.mocks.consoleError).toHaveBeenCalledWith('Metadata lookup failed:', cause);
		expect(harness.lookupState.results).toEqual([]);
		expect(harness.lookupState.hasSearched).toBe(false);
		expect(harness.lookupState.statusMessage).toBe(
			'Search failed. Check your query and try again.',
		);
	});

	it('applies a result to the current file without queue persistence', async () => {
		const result = lookupResult({ title: 'Applied Title' });
		const harness = makeHarness({
			lookupState: { results: [result], applyMode: 'current' },
			queueState: { queue: [{ file: audioFile('/books/alpha.m4b'), index: 0 }], index: 0 },
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'applyResult', index: 0 });

		expect(harness.mocks.selectFile).toHaveBeenCalledWith(
			0,
			{ multi: false, range: false },
			{ skipPersistPrevious: true },
		);
		expect(harness.mocks.applyMetadataToForm).toHaveBeenCalledWith(
			expect.objectContaining({ title: 'Applied Title', album: 'Applied Title' }),
			{ mode: 'single', markDirty: true },
		);
		expect(harness.mocks.setMetadataForFile).not.toHaveBeenCalled();
		expect(harness.mocks.updateOutputPath).toHaveBeenCalledWith('final');
		expect(harness.lookupState.statusMessage).toBe('Metadata applied to form.');
	});

	it('persists queue metadata and advances to the next selected file', async () => {
		const harness = makeHarness({
			lookupState: { results: [lookupResult()], applyMode: 'queue' },
			queueState: {
				queue: [
					{ file: audioFile('/books/alpha.m4b'), index: 0 },
					{ file: audioFile('/books/beta.m4b'), index: 1 },
				],
				index: 0,
			},
			readMetadataForm: () => ({ title: 'Alpha Patched', album: 'Alpha Patched' }),
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'applyResult', index: 0 });

		expect(harness.mocks.setMetadataForFile).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			expect.objectContaining({ title: 'Alpha Patched', album: 'Alpha Patched' }),
			expect.objectContaining({ markPending: true }),
		);
		expect(harness.queueState.index).toBe(1);
		expect(harness.lookupState.query).toBe('Beta Existing');
		expect(harness.lookupState.queueContext).toBe('2 of 2 • beta.m4b');
		expect(harness.lookupState.statusMessage).toBe('Metadata applied. Ready for next search.');
	});

	it('stores lookup-result cover art as explicit queue intent when replacement succeeds', async () => {
		const result = lookupResult({ coverUrl: 'https://example.com/cover.jpg' });
		const harness = makeHarness({
			lookupState: { results: [result], applyMode: 'queue', replaceCoverArt: true },
			queueState: {
				queue: [
					{ file: audioFile('/books/alpha.m4b'), index: 0 },
					{ file: audioFile('/books/beta.m4b'), index: 1 },
				],
				index: 0,
			},
			readMetadataForm: () => ({ title: 'Alpha Patched' }),
			loadCoverArtFromUrl: async () => [9, 9, 9],
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'applyResult', index: 0 });

		expect(harness.mocks.loadCoverArtFromUrl).toHaveBeenCalledWith('https://example.com/cover.jpg');
		expect(harness.mocks.setCustomCoverArt).toHaveBeenCalledWith([9, 9, 9]);
		expect(harness.mocks.setMetadataForFile).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			expect.objectContaining({ cover_art: [9, 9, 9] }),
			expect.objectContaining({
				intentPatch: expect.objectContaining({
					cover_art: { op: 'set', value: [9, 9, 9] },
				}),
			}),
		);
	});

	it('continues metadata apply when lookup-result cover art fails to load', async () => {
		const cause = new Error('blocked');
		const result = lookupResult({ coverUrl: 'https://example.com/cover.jpg' });
		const harness = makeHarness({
			lookupState: { results: [result], applyMode: 'queue', replaceCoverArt: true },
			queueState: {
				queue: [
					{ file: audioFile('/books/alpha.m4b'), index: 0 },
					{ file: audioFile('/books/beta.m4b'), index: 1 },
				],
				index: 0,
			},
			readMetadataForm: () => ({ title: 'Alpha Patched' }),
			loadCoverArtFromUrl: async () => {
				throw cause;
			},
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'applyResult', index: 0 });

		expect(harness.mocks.consoleWarn).toHaveBeenCalledWith(
			'Failed to load cover art from lookup:',
			cause,
		);
		expect(harness.mocks.setCustomCoverArt).not.toHaveBeenCalled();
		expect(harness.mocks.setMetadataForFile).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			expect.not.objectContaining({ cover_art: expect.anything() }),
			expect.objectContaining({
				intentPatch: expect.not.objectContaining({ cover_art: expect.anything() }),
			}),
		);
		expect(harness.lookupState.statusMessage).toBe('Metadata applied. Ready for next search.');
	});

	it('skips queue items without mutating metadata', async () => {
		const harness = makeHarness({
			queueState: {
				queue: [
					{ file: audioFile('/books/alpha.m4b'), index: 0 },
					{ file: audioFile('/books/beta.m4b'), index: 1 },
				],
				index: 0,
			},
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'skipQueueItem' });

		expect(harness.mocks.setMetadataForFile).not.toHaveBeenCalled();
		expect(harness.queueState.index).toBe(1);
		expect(harness.mocks.selectFile).toHaveBeenCalledWith(
			1,
			{ multi: false, range: false },
			{ skipPersistPrevious: true },
		);
		expect(harness.lookupState.statusMessage).toBe('Skipped. Ready for next search.');
	});

	it('restores current cover art at queue completion', async () => {
		const harness = makeHarness({
			queueState: {
				queue: [
					{ file: audioFile('/books/alpha.m4b'), index: 0 },
					{ file: audioFile('/books/beta.m4b'), index: 1 },
				],
				index: 1,
			},
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'skipQueueItem' });

		expect(harness.mocks.clearCoverArt).toHaveBeenCalledTimes(1);
		expect(harness.mocks.setCoverArt).toHaveBeenCalledWith([2]);
		expect(harness.lookupState.statusMessage).toBe('Queue complete.');
	});

	it('ignores invalid result indices', async () => {
		const harness = makeHarness({
			lookupState: { results: [] },
			queueState: { queue: [{ file: audioFile('/books/alpha.m4b'), index: 0 }], index: 0 },
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'applyResult', index: 0 });

		expect(harness.mocks.applyMetadataToForm).not.toHaveBeenCalled();
		expect(harness.mocks.setMetadataForFile).not.toHaveBeenCalled();
		expect(harness.lookupState.statusMessage).toBe('');
	});

	it('exposes typed workflow errors for rejected infrastructure calls', async () => {
		const cause = new Error('selection failed');
		const harness = makeHarness({
			queueState: {
				queue: [
					{ file: audioFile('/books/alpha.m4b'), index: 0 },
					{ file: audioFile('/books/beta.m4b'), index: 1 },
				],
				index: 0,
			},
			selectFile: async () => {
				throw cause;
			},
		});

		const error = await runAppEffect(
			Effect.flip(
				metadataLookupWorkflowExecution({ type: 'skipQueueItem' }).pipe(
					Effect.provide(harness.layer),
				),
			),
		);

		expect(error).toBeInstanceOf(MetadataLookupWorkflowFailed);
		expect(error.message).toBe('Failed to skip metadata lookup queue item.');
		expect(error.cause).toBe(cause);
	});
});
