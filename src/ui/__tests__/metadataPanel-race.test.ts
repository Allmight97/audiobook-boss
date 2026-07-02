import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import {
	autoUpdateCoverArtFromFirstValidFile,
	showMultiSelection,
	showSingleSelection,
} from '../fileList/metadataPanel';
import {
	setCurrentFileList,
	setSelectedFileIndices,
	setSelectedIndex,
} from '../fileList/state.svelte';
import { jobControlsState } from '../jobControls/state.svelte';

type Deferred<T> = {
	promise: Promise<T>;
	resolve: (value: T) => void;
	reject: (reason?: unknown) => void;
};

const context = vi.hoisted(() => ({
	readAudioMetadataMock: vi.fn(),
	getMetadataForFileMock: vi.fn(),
	setMetadataForFileMock: vi.fn(),
	populateMetadataFormSingleMock: vi.fn(),
	populateMetadataFormMultiMock: vi.fn(),
	resetDirtyStateMock: vi.fn(),
	updateEstimatedSizeMock: vi.fn(),
	updateOutputPathMock: vi.fn(),
	updateTagPreviewMock: vi.fn(),
	clearCoverArtMock: vi.fn(),
	refreshCoverArtDisplayMock: vi.fn(),
	getHasCustomCoverArtMock: vi.fn(() => false),
	setCoverArtMock: vi.fn(),
	getMetadataIntentPatchForFileMock: vi.fn(),
	renderAutoResolutionHintsMock: vi.fn(),
	resetAutoResolutionHintsMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		readAudioMetadata: context.readAudioMetadataMock,
	},
}));

vi.mock('../metadataSession', async (importOriginal) => {
	const actual = await importOriginal<typeof import('../metadataSession')>();
	return {
		...actual,
		getMetadataForFile: context.getMetadataForFileMock,
		cacheMetadataForFile: context.setMetadataForFileMock,
		getMetadataIntentPatchForFile: context.getMetadataIntentPatchForFileMock,
	};
});

vi.mock('../metadataForm', () => ({
	populateMetadataFormSingle: context.populateMetadataFormSingleMock,
	populateMetadataFormMulti: context.populateMetadataFormMultiMock,
	resetDirtyState: context.resetDirtyStateMock,
}));

vi.mock('../outputPanel', () => ({
	updateEstimatedSize: context.updateEstimatedSizeMock,
	updateOutputPath: context.updateOutputPathMock,
}));

vi.mock('../tagPreview', () => ({
	updateTagPreview: context.updateTagPreviewMock,
}));

vi.mock('../coverArt', () => ({
	clearCoverArt: context.clearCoverArtMock,
	getHasCustomCoverArt: context.getHasCustomCoverArtMock,
	setCoverArt: context.setCoverArtMock,
	refreshCoverArtDisplay: context.refreshCoverArtDisplayMock,
}));

vi.mock('../encoderPanel/autoResolutionHints', () => ({
	renderAutoResolutionHints: context.renderAutoResolutionHintsMock,
	resetAutoResolutionHints: context.resetAutoResolutionHintsMock,
}));

function createDeferred<T>(): Deferred<T> {
	let resolve!: (value: T) => void;
	let reject!: (reason?: unknown) => void;
	const promise = new Promise<T>((resolvePromise, rejectPromise) => {
		resolve = resolvePromise;
		reject = rejectPromise;
	});
	return { promise, resolve, reject };
}

async function flushAsyncWork(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
}

const makeFile = (path: string): AudioFile => ({
	path,
	isValid: true,
	size: 10,
	bitrate: 64,
	sampleRate: 44_100,
	channels: 2,
});

const makeFileList = (...files: AudioFile[]): FileListInfo => ({
	files,
	selectedDecoders: files.map(() => null),
	totalDuration: files.length,
	totalSize: files.length,
	validCount: files.filter((file) => file.isValid).length,
	invalidCount: files.filter((file) => !file.isValid).length,
});

describe('metadata panel race guards', () => {
	beforeEach(() => {
		context.readAudioMetadataMock.mockReset();
		context.getMetadataForFileMock.mockReset();
		context.setMetadataForFileMock.mockReset();
		context.populateMetadataFormSingleMock.mockReset();
		context.populateMetadataFormMultiMock.mockReset();
		context.resetDirtyStateMock.mockReset();
		context.updateEstimatedSizeMock.mockReset();
		context.updateOutputPathMock.mockReset();
		context.updateTagPreviewMock.mockReset();
		context.clearCoverArtMock.mockReset();
		context.getHasCustomCoverArtMock.mockReset();
		context.getHasCustomCoverArtMock.mockReturnValue(false);
		context.setCoverArtMock.mockReset();
		context.refreshCoverArtDisplayMock.mockReset();
		context.getMetadataIntentPatchForFileMock.mockReset();
		context.getMetadataIntentPatchForFileMock.mockReturnValue(undefined);
		context.renderAutoResolutionHintsMock.mockReset();
		context.resetAutoResolutionHintsMock.mockReset();
		context.getMetadataForFileMock.mockReturnValue(undefined);
		jobControlsState.jobType = 'merge';
		setCurrentFileList(null);
		setSelectedFileIndices([]);
		setSelectedIndex(-1);
	});

	it('ignores stale single-selection metadata that resolves after a newer selection', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');
		const alphaMetadata: Partial<AudiobookMetadata> = { title: 'Alpha' };
		const betaMetadata: Partial<AudiobookMetadata> = { title: 'Beta' };
		const alphaRequest = createDeferred<Partial<AudiobookMetadata>>();
		const betaRequest = createDeferred<Partial<AudiobookMetadata>>();

		context.readAudioMetadataMock.mockImplementation((path: string) => {
			return path.includes('alpha') ? alphaRequest.promise : betaRequest.promise;
		});

		setCurrentFileList(makeFileList(alpha, beta));

		setSelectedFileIndices([0]);
		setSelectedIndex(0);
		const alphaSelection = showSingleSelection(alpha);

		setSelectedFileIndices([1]);
		setSelectedIndex(1);
		const betaSelection = showSingleSelection(beta);

		betaRequest.resolve(betaMetadata);
		await flushAsyncWork();
		expect(context.populateMetadataFormSingleMock).toHaveBeenCalledTimes(1);
		expect(context.populateMetadataFormSingleMock).toHaveBeenLastCalledWith(betaMetadata);

		alphaRequest.resolve(alphaMetadata);
		await flushAsyncWork();
		await Promise.all([alphaSelection, betaSelection]);
		expect(context.populateMetadataFormSingleMock).toHaveBeenCalledTimes(1);
		expect(context.populateMetadataFormSingleMock).toHaveBeenLastCalledWith(betaMetadata);
	});

	it('ignores stale multi-selection metadata that resolves after switching to a single file', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');
		const gamma = makeFile('/books/gamma.m4b');
		const alphaRequest = createDeferred<Partial<AudiobookMetadata>>();
		const betaRequest = createDeferred<Partial<AudiobookMetadata>>();
		const gammaMetadata: Partial<AudiobookMetadata> = { title: 'Gamma' };

		context.readAudioMetadataMock.mockImplementation((path: string) => {
			if (path.includes('alpha')) return alphaRequest.promise;
			if (path.includes('beta')) return betaRequest.promise;
			return Promise.resolve(gammaMetadata);
		});

		setCurrentFileList(makeFileList(alpha, beta, gamma));

		setSelectedFileIndices([0, 1]);
		setSelectedIndex(0);
		const multiSelection = showMultiSelection([alpha, beta]);

		setSelectedFileIndices([2]);
		setSelectedIndex(2);
		await showSingleSelection(gamma);
		expect(context.populateMetadataFormSingleMock).toHaveBeenCalledTimes(1);
		expect(context.populateMetadataFormSingleMock).toHaveBeenLastCalledWith(gammaMetadata);

		alphaRequest.resolve({ title: 'Alpha' });
		betaRequest.resolve({ title: 'Beta' });
		await multiSelection;

		expect(context.populateMetadataFormMultiMock).not.toHaveBeenCalled();
		expect(context.populateMetadataFormSingleMock).toHaveBeenCalledTimes(1);
		expect(context.populateMetadataFormSingleMock).toHaveBeenLastCalledWith(gammaMetadata);
	});

	it('does not let a late auto-cover read overwrite custom cover art selected mid-flight', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const coverRequest = createDeferred<Partial<AudiobookMetadata>>();
		let hasCustomCoverArt = false;

		context.getHasCustomCoverArtMock.mockImplementation(() => hasCustomCoverArt);
		context.readAudioMetadataMock.mockReturnValue(coverRequest.promise);
		setCurrentFileList(makeFileList(alpha));

		const pending = autoUpdateCoverArtFromFirstValidFile();
		hasCustomCoverArt = true;

		coverRequest.resolve({ cover_art: [1, 2, 3] });
		await pending;

		expect(context.setMetadataForFileMock).not.toHaveBeenCalled();
	});

	it('drops auto-cover results from an outdated file list generation', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');
		const coverRequest = createDeferred<Partial<AudiobookMetadata>>();

		context.readAudioMetadataMock.mockReturnValue(coverRequest.promise);
		setCurrentFileList(makeFileList(alpha));

		const pending = autoUpdateCoverArtFromFirstValidFile();
		setCurrentFileList(makeFileList(beta));

		coverRequest.resolve({ cover_art: [9, 9, 9] });
		await pending;

		expect(context.setMetadataForFileMock).not.toHaveBeenCalled();
	});

	it('auto-loads cover art for the selected batch file when it is not first valid', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');
		jobControlsState.jobType = 'batch';
		context.readAudioMetadataMock.mockResolvedValue({ cover_art: [4, 5, 6] });
		setCurrentFileList(makeFileList(alpha, beta));
		setSelectedFileIndices([1]);
		setSelectedIndex(1);

		await autoUpdateCoverArtFromFirstValidFile();

		expect(context.readAudioMetadataMock).toHaveBeenCalledWith('/books/beta.m4b');
		expect(context.setMetadataForFileMock).toHaveBeenCalledWith('/books/beta.m4b', {
			cover_art: [4, 5, 6],
		});
	});

	it('loads full metadata when cache only contains cover art from auto-cover priming', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const alphaMetadata: Partial<AudiobookMetadata> = {
			title: 'Alpha Title',
			artist: 'Alpha Artist',
			cover_art: [1, 2, 3],
		};

		context.getMetadataForFileMock.mockReturnValue({ cover_art: [1, 2, 3] });
		context.readAudioMetadataMock.mockResolvedValue(alphaMetadata);
		setCurrentFileList(makeFileList(alpha));
		setSelectedFileIndices([0]);
		setSelectedIndex(0);

		await showSingleSelection(alpha);

		expect(context.readAudioMetadataMock).toHaveBeenCalledWith('/books/alpha.m4b');
		expect(context.populateMetadataFormSingleMock).toHaveBeenCalledWith(alphaMetadata);
	});

	it('loads full metadata when cache contains an empty object from auto-cover priming', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const alphaMetadata: Partial<AudiobookMetadata> = { title: 'Alpha Title' };

		context.getMetadataForFileMock.mockReturnValue({});
		context.readAudioMetadataMock.mockResolvedValue(alphaMetadata);
		setCurrentFileList(makeFileList(alpha));
		setSelectedFileIndices([0]);
		setSelectedIndex(0);

		await showSingleSelection(alpha);

		expect(context.readAudioMetadataMock).toHaveBeenCalledWith('/books/alpha.m4b');
		expect(context.populateMetadataFormSingleMock).toHaveBeenCalledWith(alphaMetadata);
	});
});
