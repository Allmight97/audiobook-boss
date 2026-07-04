import { describe, expect, it, vi } from 'vitest';

import { Effect, runAppEffect } from '../../../lib/effect/appEffect';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import { applyMetadataIntentPatch, type MetadataIntentPatch } from '../../../types/metadataIntent';
import type {
	AudiobookMetadata,
	MetadataLookupResponse,
	OnlineMetadataResult,
} from '../../../types/metadata';
import {
	fetchMetadataLookupCoverPreview,
	clearMetadataLookupCoverPreviewCache,
	scheduleMetadataLookupCoverPreviews,
} from '../metadataLookupCoverPreview.svelte';
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

function lookupResponse(
	results: OnlineMetadataResult[] = [lookupResult()],
	overrides: Partial<MetadataLookupResponse> = {},
): MetadataLookupResponse {
	return {
		results,
		diagnostics: [],
		...overrides,
	};
}

type Deferred<T> = {
	promise: Promise<T>;
	resolve: (value: T) => void;
	reject: (reason?: unknown) => void;
};

function createDeferred<T>(): Deferred<T> {
	let resolve!: (value: T) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((resolvePromise, rejectPromise) => {
		resolve = resolvePromise;
		reject = rejectPromise;
	});
	return { promise, resolve, reject };
}

async function flushAsync(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
}

function defaultLookupState(overrides: Partial<MetadataLookupState> = {}): MetadataLookupState {
	return {
		isOpen: false,
		titleQuery: '',
		authorQuery: '',
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
	const stageMetadataIntentPatch = vi.fn((filePath: string, intentPatch: MetadataIntentPatch) => {
		metadataByFile.set(
			filePath,
			applyMetadataIntentPatch(metadataByFile.get(filePath) ?? {}, intentPatch),
		);
		return 'staged' as const;
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
	const refreshCoverArtDisplay = vi.fn();
	const searchOnlineMetadata = vi.fn(
		options?.searchOnlineMetadata ?? (async () => lookupResponse()),
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
		stageMetadataIntentPatch,
		selectFile,
		applyMetadataToForm,
		readMetadataForm,
		updateOutputPath,
		updateEstimatedSize,
		updateTagPreview,
		clearCoverArt,
		setCoverArt,
		setCustomCoverArt,
		refreshCoverArtDisplay,
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
			stageMetadataIntentPatch,
			selectFile,
			applyMetadataToForm,
			readMetadataForm,
			updateOutputPath,
			updateEstimatedSize,
			updateTagPreview,
			clearCoverArt,
			setCoverArt,
			setCustomCoverArt,
			refreshCoverArtDisplay,
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
	it('reuses cached lookup cover bytes when applying metadata', async () => {
		clearMetadataLookupCoverPreviewCache();
		const result = lookupResult({ coverUrl: 'https://example.com/cover.jpg' });
		const harness = makeHarness({
			lookupState: { results: [result], replaceCoverArt: true, isOpen: true },
			queueState: {
				queue: [{ file: audioFile('/books/alpha.m4b'), index: 0 }],
				index: 0,
			},
			loadCoverArtFromUrl: async () => [9, 9, 9],
		});

		await fetchMetadataLookupCoverPreview(result.coverUrl!, harness.mocks.loadCoverArtFromUrl);
		await runMetadataLookupWorkflow(harness.layer, { type: 'applyResult', index: 0 });

		expect(harness.mocks.loadCoverArtFromUrl).toHaveBeenCalledTimes(1);
		expect(harness.mocks.setCustomCoverArt).toHaveBeenCalledWith([9, 9, 9]);
	});

	it('does not reuse cached lookup cover bytes for a different URL at the same result index', async () => {
		clearMetadataLookupCoverPreviewCache();
		const firstCoverUrl = 'https://example.com/first.jpg';
		const secondCoverUrl = 'https://example.com/second.jpg';
		const result = lookupResult({ coverUrl: secondCoverUrl });
		const loadCoverArtFromUrl = vi.fn(async (url: string) =>
			url === firstCoverUrl ? [1, 1, 1] : [2, 2, 2],
		);
		const harness = makeHarness({
			lookupState: { results: [result], replaceCoverArt: true, isOpen: true },
			queueState: {
				queue: [{ file: audioFile('/books/alpha.m4b'), index: 0 }],
				index: 0,
			},
			loadCoverArtFromUrl,
		});

		await fetchMetadataLookupCoverPreview(firstCoverUrl, harness.mocks.loadCoverArtFromUrl);
		await runMetadataLookupWorkflow(harness.layer, { type: 'applyResult', index: 0 });

		expect(harness.mocks.loadCoverArtFromUrl).toHaveBeenCalledWith(secondCoverUrl);
		expect(harness.mocks.setCustomCoverArt).toHaveBeenCalledWith([2, 2, 2]);
	});

	it('joins an in-flight eager lookup cover preview when applying metadata', async () => {
		clearMetadataLookupCoverPreviewCache();
		const result = lookupResult({ coverUrl: 'https://example.com/in-flight.jpg' });
		const request = createDeferred<number[]>();
		const loadCoverArtFromUrl = vi.fn(() => request.promise);
		const harness = makeHarness({
			lookupState: { results: [result], replaceCoverArt: true, isOpen: true },
			queueState: {
				queue: [{ file: audioFile('/books/alpha.m4b'), index: 0 }],
				index: 0,
			},
			loadCoverArtFromUrl,
		});

		scheduleMetadataLookupCoverPreviews([result.coverUrl!], harness.mocks.loadCoverArtFromUrl);
		await flushAsync();
		const pendingApply = runMetadataLookupWorkflow(harness.layer, {
			type: 'applyResult',
			index: 0,
		});
		await flushAsync();
		request.resolve([8, 8, 8]);
		await pendingApply;

		expect(harness.mocks.loadCoverArtFromUrl).toHaveBeenCalledTimes(1);
		expect(harness.mocks.setCustomCoverArt).toHaveBeenCalledWith([8, 8, 8]);
	});

	it('opens with an error when no selected files are valid', async () => {
		const harness = makeHarness({
			selectedIndices: new Set<number>([0]),
			currentFileList: fileList([audioFile('/books/invalid.m4b', false)]),
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'open' });

		expect(harness.queueState.queue).toEqual([]);
		expect(harness.lookupState.titleQuery).toBe('');
		expect(harness.lookupState.statusMessage).toBe('Select a valid file to search metadata.');
		expect(harness.lookupState.statusVariant).toBe('error');
		expect(harness.lookupState.isOpen).toBe(true);
		expect(harness.mocks.searchOnlineMetadata).not.toHaveBeenCalled();
	});

	it('opens a selected-file queue and immediately searches the first item', async () => {
		const harness = makeHarness();

		await runMetadataLookupWorkflow(harness.layer, { type: 'open' });

		expect(harness.queueState.queue.map((item) => item.file.path)).toEqual([
			'/books/alpha.m4b',
			'/books/beta.m4b',
		]);
		expect(harness.lookupState.titleQuery).toBe('Alpha Existing');
		expect(harness.lookupState.queueContext).toBe('1 of 2 • alpha.m4b');
		expect(harness.lookupState.applyMode).toBe('queue');
		expect(harness.lookupState.skipEnabled).toBe(true);
		expect(harness.mocks.searchOnlineMetadata).toHaveBeenCalledWith({
			query: 'Alpha Existing',
			sources: ['audnexus', 'openlibrary'],
			limit: 8,
		});
		expect(harness.lookupState.hasSearched).toBe(true);
		expect(harness.lookupState.statusMessage).toBe('Found 1 results.');
	});

	it('rejects empty searches without calling the backend', async () => {
		const harness = makeHarness({ lookupState: { titleQuery: '   ', authorQuery: '  ' } });

		await runMetadataLookupWorkflow(harness.layer, { type: 'search' });

		expect(harness.mocks.searchOnlineMetadata).not.toHaveBeenCalled();
		expect(harness.lookupState.statusMessage).toBe('Enter a title, author, or ASIN to search.');
		expect(harness.lookupState.statusVariant).toBe('error');
	});

	it('stores search results and sends explicit source selection to Tauri', async () => {
		const result = lookupResult({ title: 'Found Title' });
		const harness = makeHarness({
			lookupState: { titleQuery: 'alpha', source: 'auto' },
			searchOnlineMetadata: async () => lookupResponse([result]),
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

	it('surfaces degraded lookup diagnostics while keeping available results', async () => {
		const result = lookupResult({ title: 'Partial Title' });
		const harness = makeHarness({
			lookupState: { titleQuery: 'alpha', source: 'auto' },
			searchOnlineMetadata: async () =>
				lookupResponse([result], {
					diagnostics: [
						{
							kind: 'sourceFailedPartialResults',
							source: 'openlibrary',
							message: 'One selected metadata source failed; ABB is showing available results.',
						},
					],
				}),
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'search' });

		expect(harness.lookupState.results).toEqual([result]);
		expect(harness.lookupState.hasSearched).toBe(true);
		expect(harness.lookupState.statusMessage).toBe(
			'Found 1 results. Some lookup data was unavailable; showing available results.',
		);
		expect(harness.lookupState.statusVariant).toBe('info');
	});

	it('surfaces search failures without treating them as no-result matches', async () => {
		const cause = new Error('all sources failed');
		const harness = makeHarness({
			lookupState: { titleQuery: 'alpha', results: [lookupResult()], hasSearched: true },
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
		expect(harness.mocks.stageMetadataIntentPatch).not.toHaveBeenCalled();
		expect(harness.mocks.updateOutputPath).toHaveBeenCalledWith('final');
		expect(harness.lookupState.statusMessage).toBe('Metadata applied to form.');
	});

	it('persists queue metadata, advances to the next selected file, and searches it', async () => {
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

		expect(harness.mocks.stageMetadataIntentPatch).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			expect.objectContaining({
				title: { op: 'set', value: 'Alpha Patched' },
				album: { op: 'set', value: 'Alpha Patched' },
			}),
		);
		expect(harness.queueState.index).toBe(1);
		expect(harness.lookupState.titleQuery).toBe('Beta Existing');
		expect(harness.lookupState.queueContext).toBe('2 of 2 • beta.m4b');
		expect(harness.mocks.searchOnlineMetadata).toHaveBeenCalledWith({
			query: 'Beta Existing',
			sources: ['audnexus', 'openlibrary'],
			limit: 8,
		});
		expect(harness.lookupState.statusMessage).toBe('Metadata applied. Found 1 results.');
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
		expect(harness.mocks.stageMetadataIntentPatch).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			expect.objectContaining({
				cover_art: { op: 'set', value: [9, 9, 9] },
			}),
		);
		expect(harness.metadataByFile.get('/books/alpha.m4b')).toMatchObject({
			cover_art: [9, 9, 9],
		});
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
		expect(harness.mocks.stageMetadataIntentPatch).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			expect.not.objectContaining({ cover_art: expect.anything() }),
		);
		expect(harness.lookupState.statusMessage).toBe(
			'Metadata applied, but cover art failed to load. Found 1 results.',
		);
		expect(harness.lookupState.statusVariant).toBe('error');
	});

	it('reports partial success when current-file cover art replacement fails', async () => {
		const cause = new Error('blocked');
		const result = lookupResult({ coverUrl: 'https://example.com/cover.jpg' });
		const harness = makeHarness({
			lookupState: { results: [result], applyMode: 'current', replaceCoverArt: true },
			queueState: { queue: [{ file: audioFile('/books/alpha.m4b'), index: 0 }], index: 0 },
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
		expect(harness.lookupState.statusMessage).toBe(
			'Metadata applied to form, but cover art failed to load.',
		);
		expect(harness.lookupState.statusVariant).toBe('error');
	});

	it('skips queue items without mutating metadata and searches the next item', async () => {
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

		expect(harness.mocks.stageMetadataIntentPatch).not.toHaveBeenCalled();
		expect(harness.queueState.index).toBe(1);
		expect(harness.mocks.selectFile).toHaveBeenCalledWith(
			1,
			{ multi: false, range: false },
			{ skipPersistPrevious: true },
		);
		expect(harness.mocks.searchOnlineMetadata).toHaveBeenCalledWith({
			query: 'Beta Existing',
			sources: ['audnexus', 'openlibrary'],
			limit: 8,
		});
		expect(harness.lookupState.statusMessage).toBe('Skipped. Found 1 results.');
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

		expect(harness.mocks.refreshCoverArtDisplay).toHaveBeenCalledTimes(1);
		expect(harness.mocks.setCoverArt).not.toHaveBeenCalled();
		expect(harness.lookupState.statusMessage).toBe('Queue complete.');
	});

	it('ignores invalid result indices', async () => {
		const harness = makeHarness({
			lookupState: { results: [] },
			queueState: { queue: [{ file: audioFile('/books/alpha.m4b'), index: 0 }], index: 0 },
		});

		await runMetadataLookupWorkflow(harness.layer, { type: 'applyResult', index: 0 });

		expect(harness.mocks.applyMetadataToForm).not.toHaveBeenCalled();
		expect(harness.mocks.stageMetadataIntentPatch).not.toHaveBeenCalled();
		expect(harness.lookupState.statusMessage).toBe('');
	});

	it('exposes typed workflow errors for rejected infrastructure calls', async () => {
		const cause = new Error('selection failed');
		const harness = makeHarness({
			lookupState: { queueContext: '1 of 2 • alpha.m4b' },
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
		expect(harness.queueState.index).toBe(0);
		expect(harness.mocks.setMetadataLookupQueueIndex).not.toHaveBeenCalled();
		expect(harness.lookupState.queueContext).toBe('1 of 2 • alpha.m4b');
	});
});
