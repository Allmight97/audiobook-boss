import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import {
	appendFileList,
	displayFileList,
	moveFileDown,
	moveFileUp,
	reorderFiles,
	restoreImportOrder,
	toggleFileSort,
} from '../actions';
import { persistPendingMetadataDraftsForCurrentSelection } from '../metadataStaging';
import { inspectorState } from '../inspectorState.svelte';
import { showSingleSelection } from '../metadataPanel';
import {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	setCurrentFileList,
	resetImportOrder,
	setSelectedFileIndices,
	setSelectedIndex,
	setSortAscending,
} from '../state.svelte';
const context = vi.hoisted(() => ({
	readAudioMetadataMock: vi.fn(),
	getMetadataForFileMock: vi.fn(),
	stageMetadataIntentPatchMock: vi.fn(() => 'staged' as const),
	clearMetadataSessionMock: vi.fn(),
	removeMetadataForFileMock: vi.fn(),
	populateMetadataFormSingleMock: vi.fn(),
	populateMetadataFormMultiMock: vi.fn(),
	hasDirtyMetadataFieldsMock: vi.fn(() => false),
	readMetadataFormMock: vi.fn(() => ({})),
	resetDirtyStateMock: vi.fn(),
	updateEstimatedSizeMock: vi.fn(),
	updateOutputPathMock: vi.fn(),
	updateTagPreviewMock: vi.fn(),
	clearCoverArtMock: vi.fn(),
	getHasCustomCoverArtMock: vi.fn(() => false),
	refreshCoverArtDisplayMock: vi.fn(),
	setCoverArtMock: vi.fn(),
	pushStatusPanelTransientStatusMock: vi.fn(),
	renderAutoResolutionHintsMock: vi.fn(),
	resetAutoResolutionHintsMock: vi.fn(),
	validationErrorMock: vi.fn<() => string | null>(() => null),
	validateMetadataDraftMock: vi.fn(),
	metadataFormRevision: 0,
	coverArtRevision: 0,
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		readAudioMetadata: context.readAudioMetadataMock,
	},
}));

vi.mock('../../metadataSession', () => ({
	clearMetadataSession: context.clearMetadataSessionMock,
	getMetadataForFile: context.getMetadataForFileMock,
	cacheMetadataForFile: vi.fn(),
	getMetadataIntentPatchForFile: vi.fn(() => undefined),
	isUsableMetadataCache: (metadata: Record<string, unknown> | undefined) =>
		Boolean(
			metadata &&
				Object.entries(metadata).some(([key, value]) => value !== undefined && key !== 'cover_art'),
		),
	removeMetadataForFile: context.removeMetadataForFileMock,
	stageMetadataIntentPatch: context.stageMetadataIntentPatchMock,
	validateMetadataDraft: context.validateMetadataDraftMock,
	metadataSaveInProgress: { subscribe: vi.fn() },
}));

vi.mock('../../metadataForm', () => ({
	hasDirtyMetadataFields: context.hasDirtyMetadataFieldsMock,
	populateMetadataFormSingle: context.populateMetadataFormSingleMock,
	populateMetadataFormMulti: context.populateMetadataFormMultiMock,
	readMetadataForm: context.readMetadataFormMock,
	readMetadataFormRevision: vi.fn(() => context.metadataFormRevision),
	resetDirtyState: context.resetDirtyStateMock,
}));

vi.mock('../../outputPanel', () => ({
	updateEstimatedSize: context.updateEstimatedSizeMock,
	updateOutputPath: context.updateOutputPathMock,
}));

vi.mock('../../tagPreview', () => ({
	updateTagPreview: context.updateTagPreviewMock,
}));

vi.mock('../../coverArt', () => ({
	clearCoverArt: context.clearCoverArtMock,
	getHasCustomCoverArt: context.getHasCustomCoverArtMock,
	refreshCoverArtDisplay: context.refreshCoverArtDisplayMock,
	setCoverArt: context.setCoverArtMock,
	readCoverArtSessionRevision: vi.fn(() => context.coverArtRevision),
}));

vi.mock('../../statusPanel', () => ({
	initStatusPanel: vi.fn(),
	isStatusPanelProcessing: vi.fn(() => false),
	pushStatusPanelTransientStatus: context.pushStatusPanelTransientStatusMock,
}));

vi.mock('../../encoderPanel/autoResolutionHints', () => ({
	renderAutoResolutionHints: context.renderAutoResolutionHintsMock,
	resetAutoResolutionHints: context.resetAutoResolutionHintsMock,
}));

const makeFile = (path: string): AudioFile => ({
	path,
	isValid: true,
	size: 10,
	bitrate: 64,
	sampleRate: 44_100,
	channels: 2,
	codecLabel: 'AAC-LC',
	selectedDecoder: 'Apple AAC',
});

const makeFileList = (...files: AudioFile[]): FileListInfo => ({
	files,
	selectedDecoders: files.map(() => null),
	totalDuration: files.length,
	totalSize: files.length,
	validCount: files.filter((file) => file.isValid).length,
	invalidCount: files.filter((file) => !file.isValid).length,
});

describe('file list reorder behavior', () => {
	beforeEach(() => {
		context.readAudioMetadataMock.mockReset();
		context.readAudioMetadataMock.mockResolvedValue({ cover_art: null });
		context.getMetadataForFileMock.mockReset();
		context.stageMetadataIntentPatchMock.mockReset();
		context.stageMetadataIntentPatchMock.mockReturnValue('staged');
		context.clearMetadataSessionMock.mockReset();
		context.removeMetadataForFileMock.mockReset();
		context.populateMetadataFormSingleMock.mockReset();
		context.populateMetadataFormMultiMock.mockReset();
		context.hasDirtyMetadataFieldsMock.mockReset();
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.readMetadataFormMock.mockReset();
		context.readMetadataFormMock.mockReturnValue({});
		context.resetDirtyStateMock.mockReset();
		context.updateEstimatedSizeMock.mockReset();
		context.updateOutputPathMock.mockReset();
		context.updateTagPreviewMock.mockReset();
		context.validateMetadataDraftMock.mockReset();
		context.validateMetadataDraftMock.mockImplementation(
			async (metadata: Record<string, unknown>) => {
				const first = context.validationErrorMock();
				return {
					intentPatch: Object.fromEntries(
						Object.entries(metadata).map(([key, value]) => [key, { op: 'set', value }]),
					),
					ok: first == null,
					errors: { first, byField: {} },
					result: { isValid: first == null, metadataPatch: {}, fieldErrors: [] },
				};
			},
		);
		context.clearCoverArtMock.mockReset();
		context.getHasCustomCoverArtMock.mockReset();
		context.getHasCustomCoverArtMock.mockReturnValue(false);
		context.refreshCoverArtDisplayMock.mockReset();
		context.setCoverArtMock.mockReset();
		context.pushStatusPanelTransientStatusMock.mockReset();
		context.renderAutoResolutionHintsMock.mockReset();
		context.resetAutoResolutionHintsMock.mockReset();
		context.validationErrorMock.mockReset();
		context.validationErrorMock.mockReturnValue(null);
		context.metadataFormRevision = 0;
		context.coverArtRevision = 0;
		context.getMetadataForFileMock.mockReturnValue({});
		setCurrentFileList(null);
		setSelectedFileIndices([]);
		setSelectedIndex(-1);
		setSortAscending(true);
	});

	it('keeps the same file selected and updates inspector position after moving it up', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');
		const gamma = makeFile('/books/gamma.m4b');

		setCurrentFileList(makeFileList(alpha, beta, gamma));
		setSelectedFileIndices([1]);
		setSelectedIndex(1);

		await showSingleSelection(beta);
		context.populateMetadataFormSingleMock.mockClear();
		context.resetDirtyStateMock.mockClear();
		moveFileUp(1);

		expect(getSelectedFileIndex()).toBe(0);
		expect(getSelectedFileIndices()).toEqual(new Set([0]));
		expect(inspectorState.contextText).toBe('beta.m4b');
		expect(inspectorState.contextDetail).toBe('1 of 3');
		expect(context.populateMetadataFormSingleMock).not.toHaveBeenCalled();
		expect(context.resetDirtyStateMock).not.toHaveBeenCalled();
	});

	it('keeps the metadata form intact after moving a selected file down', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');
		const gamma = makeFile('/books/gamma.m4b');

		setCurrentFileList(makeFileList(alpha, beta, gamma));
		setSelectedFileIndices([1]);
		setSelectedIndex(1);

		await showSingleSelection(beta);
		context.populateMetadataFormSingleMock.mockClear();
		context.resetDirtyStateMock.mockClear();
		moveFileDown(1);

		expect(getSelectedFileIndex()).toBe(2);
		expect(getSelectedFileIndices()).toEqual(new Set([2]));
		expect(inspectorState.contextText).toBe('beta.m4b');
		expect(inspectorState.contextDetail).toBe('3 of 3');
		expect(context.populateMetadataFormSingleMock).not.toHaveBeenCalled();
		expect(context.resetDirtyStateMock).not.toHaveBeenCalled();
	});

	it('keeps the metadata form intact after drag-reordering a selected file', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');
		const gamma = makeFile('/books/gamma.m4b');

		setCurrentFileList(makeFileList(alpha, beta, gamma));
		setSelectedFileIndices([0]);
		setSelectedIndex(0);

		await showSingleSelection(alpha);
		context.populateMetadataFormSingleMock.mockClear();
		context.resetDirtyStateMock.mockClear();
		reorderFiles(0, 2);

		expect(getSelectedFileIndex()).toBe(2);
		expect(getSelectedFileIndices()).toEqual(new Set([2]));
		expect(inspectorState.contextText).toBe('alpha.m4b');
		expect(inspectorState.contextDetail).toBe('3 of 3');
		expect(context.populateMetadataFormSingleMock).not.toHaveBeenCalled();
		expect(context.resetDirtyStateMock).not.toHaveBeenCalled();
	});

	it('selects a sole imported file when a new file list is displayed', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');
		const replacement = makeFile('/books/new-alpha.m4b');

		setCurrentFileList(makeFileList(alpha, beta));
		setSelectedFileIndices([1]);
		setSelectedIndex(1);

		await showSingleSelection(beta);
		expect(inspectorState.contextText).toBe('beta.m4b');

		displayFileList(makeFileList(replacement));
		await Promise.resolve();

		expect(getSelectedFileIndex()).toBe(0);
		expect(inspectorState.contextText).toBe('new-alpha.m4b');
		expect(inspectorState.contextDetail).toBe('1 of 1');
	});

	it('renders all-file auto-resolution hints when a multi-file list has no selection', () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');

		displayFileList(makeFileList(alpha, beta));

		expect(getSelectedFileIndex()).toBe(-1);
		expect(context.renderAutoResolutionHintsMock).toHaveBeenCalledWith([alpha, beta]);
		expect(context.resetAutoResolutionHintsMock).not.toHaveBeenCalled();
	});

	it('persists the current single-file draft before additive import and keeps selection stable', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');

		setCurrentFileList(makeFileList(alpha));
		setSelectedFileIndices([0]);
		setSelectedIndex(0);
		await showSingleSelection(alpha);

		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ title: 'Draft Title' });
		context.getMetadataForFileMock.mockReturnValue({});

		expect(await persistPendingMetadataDraftsForCurrentSelection()).toBe(true);

		appendFileList(makeFileList(beta));

		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith('/books/alpha.m4b', {
			title: { op: 'set', value: 'Draft Title' },
		});
		expect(getSelectedFileIndex()).toBe(0);
		expect(inspectorState.contextText).toBe('alpha.m4b');
	});

	it('appends using explicit existing files when the caller supplies the visible list', () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');

		setCurrentFileList(null);

		appendFileList(makeFileList(beta), { existingFiles: [alpha] });

		expect(getCurrentFileList()?.files.map((file) => file.path)).toEqual([
			'/books/alpha.m4b',
			'/books/beta.m4b',
		]);
	});

	it('stages selected metadata drafts before sorting and preserves selection identity', async () => {
		const alpha = makeFile('/books/a-alpha.m4b');
		const beta = makeFile('/books/b-beta.m4b');

		setCurrentFileList(makeFileList(beta, alpha));
		setSelectedFileIndices([0, 1]);
		setSelectedIndex(0);
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ series: 'Sorted Series' });

		await toggleFileSort();

		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith('/books/a-alpha.m4b', {
			series: { op: 'set', value: 'Sorted Series' },
		});
		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith('/books/b-beta.m4b', {
			series: { op: 'set', value: 'Sorted Series' },
		});
		expect(getSelectedFileIndex()).toBe(1);
		expect(getCurrentFileList()?.files.map((file) => file.path)).toEqual([
			'/books/a-alpha.m4b',
			'/books/b-beta.m4b',
		]);
	});

	it('blocks sorting when selected metadata drafts fail validation', async () => {
		const alpha = makeFile('/books/b-alpha.m4b');
		const beta = makeFile('/books/a-beta.m4b');

		setCurrentFileList(makeFileList(alpha, beta));
		setSelectedFileIndices([0, 1]);
		setSelectedIndex(0);
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ seriesPart: 'bad' });
		context.validationErrorMock.mockReturnValue('Series part must be a number');

		await toggleFileSort();

		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
		expect(context.pushStatusPanelTransientStatusMock).toHaveBeenCalledWith(
			'Fix metadata validation errors before sorting files.',
			expect.objectContaining({ ttlMs: 2500 }),
		);
		expect(getSelectedFileIndex()).toBe(0);
		expect(getCurrentFileList()?.files.map((file) => file.path)).toEqual([
			'/books/b-alpha.m4b',
			'/books/a-beta.m4b',
		]);
	});

	it('lets the latest sort or restore intent win when validation resolves out of order', async () => {
		type ValidationResult = {
			intentPatch: Record<string, unknown>;
			ok: boolean;
			errors: { first: null; byField: Record<string, unknown> };
			result: { isValid: boolean; metadataPatch: Record<string, unknown>; fieldErrors: never[] };
		};
		type Deferred<T> = { promise: Promise<T>; resolve: (value: T) => void };
		const deferred = <T>(): Deferred<T> => {
			let resolve!: (value: T) => void;
			const promise = new Promise<T>((resolvePromise) => {
				resolve = resolvePromise;
			});
			return { promise, resolve };
		};
		const firstValidation = deferred<ValidationResult>();
		const secondValidation = deferred<ValidationResult>();
		context.validateMetadataDraftMock
			.mockImplementationOnce(() => firstValidation.promise)
			.mockImplementationOnce(() => secondValidation.promise);

		const beta = makeFile('/books/b-beta.m4b');
		const alpha = makeFile('/books/a-alpha.m4b');
		setCurrentFileList(makeFileList(beta, alpha));
		resetImportOrder([beta, alpha]);
		setSelectedFileIndices([0, 1]);
		setSelectedIndex(0);
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ series: 'Draft Series' });

		const first = toggleFileSort();
		const second = restoreImportOrder();

		secondValidation.resolve({
			intentPatch: { series: { op: 'set', value: 'Latest' } },
			ok: true,
			errors: { first: null, byField: {} },
			result: { isValid: true, metadataPatch: {}, fieldErrors: [] },
		});
		await second;
		firstValidation.resolve({
			intentPatch: { series: { op: 'set', value: 'Stale' } },
			ok: true,
			errors: { first: null, byField: {} },
			result: { isValid: true, metadataPatch: {}, fieldErrors: [] },
		});
		await first;

		expect(getCurrentFileList()?.files.map((file) => file.path)).toEqual([
			'/books/b-beta.m4b',
			'/books/a-alpha.m4b',
		]);
		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledTimes(2);
		expect(getSelectedFileIndex()).toBe(0);
	});
});
