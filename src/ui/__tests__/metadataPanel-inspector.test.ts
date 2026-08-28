import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../types/audio';
import {
	clearSelectionPanels,
	showMultiSelection,
	showSingleSelection,
} from '../fileList/metadataPanel';
import { setCurrentFileList, setSelectedIndex } from '../fileList/state';
import { inspectorState } from '../fileList/inspectorState';

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
	getHasCustomCoverArtMock: vi.fn(() => false),
	setCoverArtMock: vi.fn(),
	companionSummaryForInputIdsMock: vi.fn(),
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
}));

vi.mock('../remoteSource/sessionAssets.svelte', () => ({
	companionSummaryForInputIds: context.companionSummaryForInputIdsMock,
}));

const makeFile = (overrides: Partial<AudioFile>): AudioFile => ({
	path: '/books/input.m4b',
	isValid: true,
	size: 10 * 1024 * 1024,
	bitrate: 64,
	sampleRate: 44100,
	channels: 2,
	...overrides,
});

const makeFileList = (...files: AudioFile[]): FileListInfo => ({
	files,
	selectedDecoders: files.map(() => null),
	totalDuration: files.length,
	totalSize: files.length,
	validCount: files.filter((file) => file.isValid).length,
	invalidCount: files.filter((file) => !file.isValid).length,
});

describe('metadata panel input inspector', () => {
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
		context.setCoverArtMock.mockReset();
		context.companionSummaryForInputIdsMock.mockReset();
		context.companionSummaryForInputIdsMock.mockReturnValue({
			text: 'None',
			title: '',
			pdfCount: 0,
			fileCountWithCompanions: 0,
		});
		context.getMetadataForFileMock.mockReturnValue({});
		setSelectedIndex(-1);
	});

	it('shows codec and decoder for a single selected file', async () => {
		const file = makeFile({
			path: '/books/science.m4b',
			codecLabel: 'USAC / xHE-AAC',
			selectedDecoder: 'Apple AAC',
		});
		setCurrentFileList(makeFileList(file));
		setSelectedIndex(0);

		await showSingleSelection(file);

		expect(inspectorState.codecText).toBe('USAC / xHE-AAC');
		expect(inspectorState.decoderText).toBe('Apple AAC');
		expect(inspectorState.contextText).toContain('science.m4b');
	});

	it('shows companion assets for a single selected file', async () => {
		const pdfFileName = 'Being You - A New Science of Consciousness - Supplemental PDF.pdf';
		context.companionSummaryForInputIdsMock.mockReturnValueOnce({
			text: pdfFileName,
			title: pdfFileName,
			pdfCount: 1,
			fileCountWithCompanions: 1,
		});
		const file = makeFile({
			inputId: 'current-input-1',
			path: '/books/science.m4b',
		});
		setCurrentFileList(makeFileList(file));
		setSelectedIndex(0);

		await showSingleSelection(file);

		expect(context.companionSummaryForInputIdsMock).toHaveBeenCalledWith(['current-input-1']);
		expect(inspectorState.companionsText).toBe(pdfFileName);
		expect(inspectorState.companionsTitle).toBe(pdfFileName);
	});

	it('shows shared codec and decoder values for multi-selection when they match', async () => {
		const first = makeFile({
			path: '/books/one.m4b',
			codecLabel: 'AAC-LC',
			selectedDecoder: 'Native AAC (FFmpeg)',
		});
		const second = makeFile({
			path: '/books/two.m4b',
			codecLabel: 'AAC-LC',
			selectedDecoder: 'Native AAC (FFmpeg)',
		});
		setCurrentFileList(makeFileList(first, second));

		await showMultiSelection([first, second]);

		expect(inspectorState.codecText).toBe('AAC-LC');
		expect(inspectorState.decoderText).toBe('Native AAC (FFmpeg)');
		expect(inspectorState.contextText).toContain('2 files selected');
	});

	it('shows Mixed when codec or decoder values differ or are partially unknown', async () => {
		const first = makeFile({
			path: '/books/one.m4b',
			codecLabel: 'AAC-LC',
			selectedDecoder: 'Native AAC (FFmpeg)',
		});
		const second = makeFile({
			path: '/books/two.m4b',
			codecLabel: undefined,
			selectedDecoder: 'Apple AAC',
		});
		setCurrentFileList(makeFileList(first, second));

		await showMultiSelection([first, second]);

		expect(inspectorState.codecText).toBe('Mixed');
		expect(inspectorState.decoderText).toBe('Mixed');
	});

	it('clears codec and decoder rows back to placeholders', () => {
		clearSelectionPanels();

		expect(inspectorState.codecText).toBe('---');
		expect(inspectorState.decoderText).toBe('---');
		expect(inspectorState.companionsText).toBe('---');
	});
});
