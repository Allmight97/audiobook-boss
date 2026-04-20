import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../types/audio';
import {
	appendFileList,
	displayFileList,
	moveFileUp,
	persistPendingMetadataDraftsForCurrentSelection,
} from '../fileList/actions';
import { inspectorState } from '../fileList/inspectorState.svelte';
import { showSingleSelection } from '../fileList/metadataPanel';
import {
	getCurrentFileList,
	getSelectedFileIndex,
	setCurrentFileList,
	setSelectedFileIndices,
	setSelectedIndex,
} from '../fileList/state';
import { resetFileListViewState } from '../fileList/viewState.svelte';

const context = vi.hoisted(() => ({
	readAudioMetadataMock: vi.fn(),
	getMetadataForFileMock: vi.fn(),
	setMetadataForFileMock: vi.fn(),
	clearMetadataStateMock: vi.fn(),
	removeMetadataForFileMock: vi.fn(),
	metadataEqualsNullishMock: vi.fn(() => false),
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
	setCoverArtMock: vi.fn(),
	pushStatusPanelTransientStatusMock: vi.fn(),
	renderAutoResolutionHintsMock: vi.fn(),
	resetAutoResolutionHintsMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		readAudioMetadata: context.readAudioMetadataMock,
	},
}));

vi.mock('../metadataState', () => ({
	clearMetadataState: context.clearMetadataStateMock,
	getMetadataForFile: context.getMetadataForFileMock,
	metadataEqualsNullish: context.metadataEqualsNullishMock,
	removeMetadataForFile: context.removeMetadataForFileMock,
	setMetadataForFile: context.setMetadataForFileMock,
}));

vi.mock('../metadataForm', () => ({
	hasDirtyMetadataFields: context.hasDirtyMetadataFieldsMock,
	populateMetadataFormSingle: context.populateMetadataFormSingleMock,
	populateMetadataFormMulti: context.populateMetadataFormMultiMock,
	readMetadataForm: context.readMetadataFormMock,
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
}));

vi.mock('../statusPanel', () => ({
	pushStatusPanelTransientStatus: context.pushStatusPanelTransientStatusMock,
}));

vi.mock('../metadataValidation', () => ({
	getSeriesPartValidationError: vi.fn(() => null),
	getSubseriesPartValidationError: vi.fn(() => null),
}));

vi.mock('../fileList/dom', () => ({
	initDOMCache: vi.fn(),
	updateFileListDOM: vi.fn(),
	updateTotalStats: vi.fn(),
	updateSelection: vi.fn(),
	updateSortButtonText: vi.fn(),
	updateButtonVisibility: vi.fn(),
	showEmptyState: vi.fn(),
	setOrderLockNotice: vi.fn(),
}));

vi.mock('../encoderPanel/autoResolutionHints', () => ({
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
		context.setMetadataForFileMock.mockReset();
		context.clearMetadataStateMock.mockReset();
		context.removeMetadataForFileMock.mockReset();
		context.metadataEqualsNullishMock.mockReset();
		context.metadataEqualsNullishMock.mockReturnValue(false);
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
		context.clearCoverArtMock.mockReset();
		context.getHasCustomCoverArtMock.mockReset();
		context.getHasCustomCoverArtMock.mockReturnValue(false);
		context.setCoverArtMock.mockReset();
		context.pushStatusPanelTransientStatusMock.mockReset();
		context.renderAutoResolutionHintsMock.mockReset();
		context.resetAutoResolutionHintsMock.mockReset();
		context.getMetadataForFileMock.mockReturnValue({});
		setCurrentFileList(null);
		setSelectedFileIndices([]);
		setSelectedIndex(-1);
		resetFileListViewState();
	});

	it('keeps the same file selected and updates inspector position after moving it up', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');
		const gamma = makeFile('/books/gamma.m4b');

		setCurrentFileList(makeFileList(alpha, beta, gamma));
		setSelectedFileIndices([1]);
		setSelectedIndex(1);

		await showSingleSelection(beta);
		moveFileUp(1);

		expect(getSelectedFileIndex()).toBe(0);
		expect(inspectorState.contextText).toBe('beta.m4b');
		expect(inspectorState.contextDetail).toBe('1 of 3');
	});

	it('clears inspector context when a new file list is displayed', async () => {
		const alpha = makeFile('/books/alpha.m4b');
		const beta = makeFile('/books/beta.m4b');

		setCurrentFileList(makeFileList(alpha, beta));
		setSelectedFileIndices([1]);
		setSelectedIndex(1);

		await showSingleSelection(beta);
		expect(inspectorState.contextText).toBe('beta.m4b');

		displayFileList(makeFileList(makeFile('/books/new-alpha.m4b')));

		expect(getSelectedFileIndex()).toBe(-1);
		expect(inspectorState.contextText).toBe('No file selected');
		expect(inspectorState.contextDetail).toBe('');
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

		expect(context.setMetadataForFileMock).toHaveBeenCalledWith(
			'/books/alpha.m4b',
			{ title: 'Draft Title' },
			{
				markPending: true,
				intentPatch: { title: { op: 'set', value: 'Draft Title' } },
			},
		);
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
});
