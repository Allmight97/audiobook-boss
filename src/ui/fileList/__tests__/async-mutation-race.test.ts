import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import {
	removeSelectedFiles,
	removeFile,
	toggleFileSort,
	restoreImportOrder,
	setFileOrderLocked,
	appendFileList,
	displayFileList,
} from '../actions';
import {
	getCurrentFileList,
	setCurrentFileList,
	setSelectedFileIndices,
	setSelectedIndex,
	resetImportOrder,
} from '../state.svelte';

const context = vi.hoisted(() => ({
	hasDirtyMetadataFieldsMock: vi.fn(() => false),
	readMetadataFormMock: vi.fn(() => ({})),
	validateMetadataDraftMock: vi.fn(async () => ({ ok: true as const, intentPatch: {} })),
	stageMetadataIntentPatchMock: vi.fn(() => 'staged' as const),
	cacheMetadataForFileMock: vi.fn(),
	getMetadataForFileMock: vi.fn(() => undefined),
	removeMetadataForFileMock: vi.fn(),
	clearMetadataSessionMock: vi.fn(),
	updateEstimatedSizeMock: vi.fn(),
	updateOutputPathMock: vi.fn(),
	pushStatusPanelTransientStatusMock: vi.fn(),
	readAudioMetadataMock: vi.fn(),
	prepareResolve: null as (() => void) | null,
	prepareStarted: false,
	metadataReadResolve: null as (() => void) | null,
	metadataReadStarted: false,
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		readAudioMetadata: context.readAudioMetadataMock,
		validateMetadataIntentPatch: vi.fn(),
	},
}));

vi.mock('../../metadataSession', () => ({
	clearMetadataSession: context.clearMetadataSessionMock,
	getMetadataForFile: context.getMetadataForFileMock,
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
		setFileOrderLocked(false);
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.stageMetadataIntentPatchMock.mockClear();
		context.cacheMetadataForFileMock.mockClear();
		context.prepareResolve = null;
		context.prepareStarted = false;
		context.metadataReadResolve = null;
		context.metadataReadStarted = false;
		context.readMetadataFormMock.mockReturnValue({ title: 'Draft' });

		context.validateMetadataDraftMock.mockImplementation(async () => {
			context.prepareStarted = true;
			await new Promise<void>((resolve) => {
				context.prepareResolve = resolve;
			});
			return { ok: true as const, intentPatch: { title: { op: 'set' as const, value: 'Draft' } } };
		});

		context.readAudioMetadataMock.mockImplementation(async () => {
			context.metadataReadStarted = true;
			await new Promise<void>((resolve) => {
				context.metadataReadResolve = resolve;
			});
			return { title: 'Cached' };
		});

		const alpha = makeFile('/books/alpha.m4b', 'id-alpha');
		const beta = makeFile('/books/beta.m4b', 'id-beta');
		setCurrentFileList(makeFileList(alpha, beta));
		resetImportOrder([alpha, beta]);
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
		expect(context.cacheMetadataForFileMock).not.toHaveBeenCalled();
	});

	it('removeSelectedFiles aborts identity remove when revision changes during prepare', async () => {
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

	it('multi-select does not cache/stage when list mutates during prepare', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		setSelectedFileIndices([0, 1]);
		setSelectedIndex(0);
		context.readAudioMetadataMock.mockResolvedValue({ title: 'Cached' });

		const sortPromise = toggleFileSort();
		await vi.waitFor(() => expect(context.prepareStarted).toBe(true));

		displayFileList(makeFileList(makeFile('/books/replaced.m4b', 'id-replaced')));
		context.prepareResolve?.();
		await sortPromise;

		expect(getCurrentFileList()?.files.map((file) => file.path)).toEqual(['/books/replaced.m4b']);
		expect(context.cacheMetadataForFileMock).not.toHaveBeenCalled();
		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
	});

	it('toggleFileSort does not clobber a concurrent append after prepare', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		const sortPromise = toggleFileSort();

		await vi.waitFor(() => expect(context.prepareStarted).toBe(true));

		const gamma = makeFile('/books/gamma.m4b', 'id-gamma');
		appendFileList(makeFileList(gamma), { showDuplicateStatus: false });
		context.prepareResolve?.();
		await sortPromise;

		const paths = getCurrentFileList()?.files.map((file) => file.path) ?? [];
		expect(paths).toContain('/books/gamma.m4b');
		expect(paths).toHaveLength(3);
		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
	});

	it('restoreImportOrder aborts when revision changes during prepare', async () => {
		const beta = makeFile('/books/beta.m4b', 'id-beta');
		const alpha = makeFile('/books/alpha.m4b', 'id-alpha');
		displayFileList(makeFileList(beta, alpha));
		setSelectedIndex(0);
		setSelectedFileIndices([0]);
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);

		const restorePromise = restoreImportOrder();
		await vi.waitFor(() => expect(context.prepareStarted).toBe(true));

		displayFileList(makeFileList(makeFile('/books/replaced.m4b', 'id-replaced')));
		context.prepareResolve?.();
		await restorePromise;

		expect(getCurrentFileList()?.files.map((file) => file.path)).toEqual(['/books/replaced.m4b']);
		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
	});
});
