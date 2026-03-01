import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../types/audio';
import {
	clearSelectionPanels,
	showMultiSelection,
	showSingleSelection,
} from '../fileList/metadataPanel';
import { setCurrentFileList, setSelectedIndex } from '../fileList/state';

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
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		readAudioMetadata: context.readAudioMetadataMock,
	},
}));

vi.mock('../metadataState', () => ({
	getMetadataForFile: context.getMetadataForFileMock,
	setMetadataForFile: context.setMetadataForFileMock,
}));

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
	totalDuration: files.length,
	totalSize: files.length,
	validCount: files.filter((file) => file.isValid).length,
	invalidCount: files.filter((file) => !file.isValid).length,
});

const text = (id: string): string => document.getElementById(id)?.textContent ?? '';

describe('metadata panel input inspector', () => {
	beforeEach(() => {
		document.body.innerHTML = `
			<div id="prop-selected-context"></div>
			<span id="prop-bitrate">---</span>
			<span id="prop-samplerate">---</span>
			<span id="prop-channels">---</span>
			<span id="prop-codec">---</span>
			<span id="prop-decoder">---</span>
			<span id="prop-filesize">---</span>
			<span id="output-samplerate-effective"></span>
			<span id="output-channels-effective"></span>
		`;
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

		expect(text('prop-codec')).toBe('USAC / xHE-AAC');
		expect(text('prop-decoder')).toBe('Apple AAC');
		expect(text('prop-selected-context')).toContain('science.m4b');
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

		expect(text('prop-codec')).toBe('AAC-LC');
		expect(text('prop-decoder')).toBe('Native AAC (FFmpeg)');
		expect(text('prop-selected-context')).toContain('2 files selected');
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

		expect(text('prop-codec')).toBe('Mixed');
		expect(text('prop-decoder')).toBe('Mixed');
	});

	it('clears codec and decoder rows back to placeholders', () => {
		document.getElementById('prop-codec')!.textContent = 'AAC-LC';
		document.getElementById('prop-decoder')!.textContent = 'Apple AAC';

		clearSelectionPanels();

		expect(text('prop-codec')).toBe('---');
		expect(text('prop-decoder')).toBe('---');
	});
});
