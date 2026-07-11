import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../types/audio';
import {
	clearSelectionPanels,
	makeMetadataSurfaceTransitionCoordinator,
	showMultiSelection,
	showSingleSelection,
} from '../fileList/metadataPanel';
import { readInspectorFacts } from '../fileList/inspectorState.svelte';
import { setCurrentFileList, setSelectedIndex } from '../fileList/state.svelte';
import { inspectorState } from '../fileList/inspectorState.svelte';

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
		expect(readInspectorFacts().map((fact) => fact.label)).toEqual([
			'File',
			'Position',
			'Bitrate',
			'Sample rate',
			'Channels',
			'Codec',
			'Decoder',
			'File size',
			'Supplemental',
			'Combined size',
		]);
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

describe('metadata surface transition coordinator', () => {
	it('stages the old draft, closes without staging again, then mutates and populates', async () => {
		const calls: string[] = [];
		const row = document.createElement('button');
		const coordinator = makeMetadataSurfaceTransitionCoordinator({
			persistOldDrafts: async () => {
				calls.push('stage-old');
				return true;
			},
			getSelectedFiles: () => [{ path: '/books/new.m4b', isValid: true }],
			populateSelection: async () => {
				calls.push('populate-new');
			},
			closeWithoutStaging: () => calls.push('close'),
			open: (anchor) => calls.push(anchor === row ? 'open-row' : 'wrong-anchor'),
			getActiveRowControl: () => row,
		});

		await coordinator.selection(
			{ type: 'selectOnly', index: 1 },
			() => {
				calls.push('mutate');
				return { changed: true };
			},
			{ openAfterPopulate: true },
		);

		expect(calls).toEqual(['stage-old', 'close', 'mutate', 'populate-new', 'open-row']);
	});

	it('keeps the popover open and aborts mutation when validation rejects staging', async () => {
		const mutate = vi.fn(() => ({ changed: true }));
		const close = vi.fn();
		const coordinator = makeMetadataSurfaceTransitionCoordinator({
			persistOldDrafts: vi.fn(async () => false),
			getSelectedFiles: () => [],
			populateSelection: vi.fn(),
			closeWithoutStaging: close,
			open: vi.fn(),
			getActiveRowControl: () => null,
		});

		await expect(coordinator.selection({ type: 'toggle', index: 1 }, mutate)).resolves.toBe(false);
		expect(close).not.toHaveBeenCalled();
		expect(mutate).not.toHaveBeenCalled();
	});

	it('serializes rapid selection intents without double-staging a dirty draft', async () => {
		let dirty = true;
		const stageIntentPatch = vi.fn();
		const coordinator = makeMetadataSurfaceTransitionCoordinator({
			persistOldDrafts: async () => {
				if (dirty) stageIntentPatch('/books/old.m4b');
				return true;
			},
			getSelectedFiles: () => [{ path: '/books/new.m4b', isValid: true }],
			populateSelection: async () => {},
			closeWithoutStaging: vi.fn(),
			open: vi.fn(),
			getActiveRowControl: () => null,
		});
		const mutate = () => {
			dirty = false;
			return { changed: true };
		};

		await Promise.all([
			coordinator.selection({ type: 'selectOnly', index: 1 }, mutate),
			coordinator.selection({ type: 'selectOnly', index: 2 }, mutate),
		]);

		expect(stageIntentPatch).toHaveBeenCalledTimes(1);
		expect(stageIntentPatch).toHaveBeenCalledWith('/books/old.m4b');
	});

	it('stages and closes a click-away dismissal exactly once', async () => {
		const persistOldDrafts = vi.fn(async () => true);
		const closeWithoutStaging = vi.fn();
		const coordinator = makeMetadataSurfaceTransitionCoordinator({
			persistOldDrafts,
			getSelectedFiles: () => [],
			populateSelection: vi.fn(),
			closeWithoutStaging,
			open: vi.fn(),
			getActiveRowControl: () => null,
		});

		await expect(coordinator.dismiss()).resolves.toBe(true);
		expect(persistOldDrafts).toHaveBeenCalledOnce();
		expect(persistOldDrafts).toHaveBeenCalledWith({ showStatus: true });
		expect(closeWithoutStaging).toHaveBeenCalledOnce();
	});
});
