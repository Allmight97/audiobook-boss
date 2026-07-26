import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import {
	removeSelectedFiles,
	removeFile,
	toggleFileSort,
	setFileOrderLocked,
} from '../actions';
import {
	getCurrentFileList,
	setCurrentFileList,
	setSelectedFileIndices,
	setSelectedIndex,
} from '../state.svelte';

const context = vi.hoisted(() => ({
	hasDirtyMetadataFieldsMock: vi.fn(() => false),
	readMetadataFormMock: vi.fn(() => ({})),
	validateMetadataDraftMock: vi.fn(async () => ({ ok: true as const, intentPatch: {} })),
	stageMetadataIntentPatchMock: vi.fn(() => 'staged' as const),
	cacheMetadataForFileMock: vi.fn(),
	removeMetadataForFileMock: vi.fn(),
	clearMetadataSessionMock: vi.fn(),
	updateEstimatedSizeMock: vi.fn(),
	updateOutputPathMock: vi.fn(),
	pushStatusPanelTransientStatusMock: vi.fn(),
	readAudioMetadataMock: vi.fn(),
	ensureMetadataForFilesMock: vi.fn(async () => undefined),
	prepareResolve: null as (() => void) | null,
	prepareStarted: false,
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		readAudioMetadata: context.readAudioMetadataMock,
		validateMetadataIntentPatch: vi.fn(),
	},
}));

vi.mock('../../metadataSession', () => ({
	clearMetadataSession: context.clearMetadataSessionMock,
	getMetadataForFile: vi.fn(() => undefined),
	cacheMetadataForFile: context.cacheMetadataForFileMock,
	getMetadataIntentPatchForFile: vi.fn(() => undefined),
	isUsableMetadataCache: () => false,
	removeMetadataForFile: context.removeMetadataForFileMock,
	stageMetadataIntentPatch: context.stageMetadataIntentPatchMock,
	validateMetadataDraft: context.validateMetadataDraftMock,
	metadataSaveInProgress: { subscribe: vi.fn() },
}));

vi.mock('../../metadataForm', () => ({
	hasDirtyMetadataFields: context.hasDirtyMetadataFieldsMock,
	readMetadataForm: context.readMetadataFormMock,
	resetDirtyState: vi.fn(),
	populateMetadataFormSingle: vi.fn(),
	populateMetadataFormMulti: vi.fn(),
}));

vi.mock('../../outputPanel', () => ({
	updateEstimatedSize: context.updateEstimatedSizeMock,
	updateOutputPath: context.updateOutputPathMock,
}));

vi.mock('../../statusPanel', () => ({
	pushStatusPanelTransientStatus: context.pushStatusPanelTransientStatusMock,
}));

vi.mock('../../tagPreview', () => ({ updateTagPreview: vi.fn() }));
vi.mock('../../coverArt', () => ({
	clearCoverArt: vi.fn(),
	getHasCustomCoverArt: vi.fn(() => false),
	refreshCoverArtDisplay: vi.fn(),
	setCoverArt: vi.fn(),
}));
vi.mock('../../encoderPanel/autoResolutionHints', () => ({
	renderAutoResolutionHints: vi.fn(),
	resetAutoResolutionHints: vi.fn(),
}));
vi.mock('../../remoteSource', () => ({
	companionSummaryForInputIds: vi.fn(() => ({ text: '---', title: '' })),
	purgeRemoteSourceSessionsForInputIds: vi.fn(),
}));
vi.mock('../coverThumbnails.svelte', () => ({
	removeFileListCoverThumbnail: vi.fn(),
}));

vi.mock('../metadataPanel', async (importOriginal) => {
	const actual = await importOriginal<typeof import('../metadataPanel')>();
	return {
		...actual,
		ensureMetadataForFiles: context.ensureMetadataForFilesMock,
		showSingleSelection: vi.fn(),
		showMultiSelection: vi.fn(),
		clearSelectionPanels: vi.fn(),
		coordinateMetadataSurfacePresentationRefresh: vi.fn(),
	};
});

function makeFile(path: string, inputId?: string): AudioFile {
	return {
		path,
		inputId: inputId ?? path,
		size: 1,
		duration: 1,
		isValid: true,
	};
}

function makeFileList(...files: AudioFile[]): FileListInfo {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length,
		totalSize: files.length,
		validCount: files.length,
		invalidCount: 0,
	};
}

describe('FileList async mutation races', () => {
	beforeEach(() => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.stageMetadataIntentPatchMock.mockClear();
		context.cacheMetadataForFileMock.mockClear();
		context.prepareResolve = null;
		context.prepareStarted = false;

		context.validateMetadataDraftMock.mockImplementation(async () => {
			context.prepareStarted = true;
			await new Promise<void>((resolve) => {
				context.prepareResolve = resolve;
			});
			return { ok: true as const, intentPatch: { title: { op: 'set' as const, value: 'Draft' } } };
		});

		const alpha = makeFile('/books/alpha.m4b', 'id-alpha');
		const beta = makeFile('/books/beta.m4b', 'id-beta');
		setCurrentFileList(makeFileList(alpha, beta));
		setSelectedIndex(0);
		setSelectedFileIndices([0]);
	});

	it('removeSelectedFiles aborts when the list changes during draft prepare', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		const removePromise = removeSelectedFiles();

		await vi.waitFor(() => expect(context.prepareStarted).toBe(true));

		removeFile(1);
		context.prepareResolve?.();

		await removePromise;

		expect(getCurrentFileList()?.files.map((file) => file.path)).toEqual(['/books/alpha.m4b']);
		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
	});

	it('removeSelectedFiles removes by identity key after prepare, not stale indices', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		const removePromise = removeSelectedFiles();

		await vi.waitFor(() => expect(context.prepareStarted).toBe(true));

		removeFile(0);
		context.prepareResolve?.();

		await removePromise;

		expect(getCurrentFileList()?.files.map((file) => file.path)).toEqual(['/books/beta.m4b']);
		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
	});

	it('removeSelectedFiles commits draft only when revision is still current', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		const removePromise = removeSelectedFiles();

		await vi.waitFor(() => expect(context.prepareStarted).toBe(true));
		context.prepareResolve?.();
		await removePromise;

		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith('/books/alpha.m4b', {
			title: { op: 'set', value: 'Draft' },
		});
		expect(getCurrentFileList()?.files.map((file) => file.path)).toEqual(['/books/beta.m4b']);
	});

	it('toggleFileSort aborts when order lock engages during draft prepare', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		const sortPromise = toggleFileSort();

		await vi.waitFor(() => expect(context.prepareStarted).toBe(true));

		setFileOrderLocked(true);
		context.prepareResolve?.();

		await sortPromise;

		expect(getCurrentFileList()?.files.map((file) => file.path)).toEqual([
			'/books/alpha.m4b',
			'/books/beta.m4b',
		]);
		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
	});
});
