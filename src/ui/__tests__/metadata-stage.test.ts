import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	persistPendingMetadataDraftsForCurrentSelection,
	setCurrentFileList,
	setSelectedFileIndices,
	stageMetadataToSelection,
} from '../fileList';
import {
	clearMetadataState,
	getMetadataForFile,
	getPendingMetadataEntries,
	setMetadataForFile,
} from '../metadataState';
import type { FileListInfo } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';

const context = vi.hoisted(() => ({
	validationErrorMock: vi.fn<() => string | null>(() => null),
	validateMetadataDraftIntentMock: vi.fn(async (metadata: Partial<AudiobookMetadata>) => ({
		intentPatch: Object.fromEntries(
			Object.entries(metadata).map(([key, value]) => [
				key,
				value === '' || value === undefined ? { op: 'clear' } : { op: 'set', value },
			]),
		),
		result: {
			isValid: true,
			metadataPatch: {},
			fieldErrors: [],
		},
	})),
	selectedFilesMock: vi.fn(() => [
		{ path: '/a.mp3', isValid: true },
		{ path: '/b.mp3', isValid: true },
	]),
	readMetadataFormMock: vi.fn<() => Partial<AudiobookMetadata>>(() => ({ series: 'Series X' })),
	hasDirtyMetadataFieldsMock: vi.fn(() => false),
}));

vi.mock('../fileList/metadataPanel', () => ({
	ensureMetadataForFiles: vi.fn(async () => undefined),
	getSelectedFiles: context.selectedFilesMock,
}));

vi.mock('../outputPanel', () => ({
	updateEstimatedSize: vi.fn(),
	updateOutputPath: vi.fn(),
}));

vi.mock('../metadataValidation', () => ({
	firstMetadataIntentValidationError: context.validationErrorMock,
	validateMetadataDraftIntent: context.validateMetadataDraftIntentMock,
}));

vi.mock('../metadataForm', () => ({
	readMetadataForm: context.readMetadataFormMock,
	hasDirtyMetadataFields: context.hasDirtyMetadataFieldsMock,
	resetDirtyState: vi.fn(),
}));

describe('stageMetadataToSelection', () => {
	beforeEach(() => {
		document.body.innerHTML = '';
		clearMetadataState();
		context.validationErrorMock.mockReset();
		context.validateMetadataDraftIntentMock.mockClear();
		context.selectedFilesMock.mockReset();
		context.readMetadataFormMock.mockReset();
		context.hasDirtyMetadataFieldsMock.mockReset();
		context.validationErrorMock.mockReturnValue(null);
		context.selectedFilesMock.mockReturnValue([
			{ path: '/a.mp3', isValid: true },
			{ path: '/b.mp3', isValid: true },
		]);
		context.readMetadataFormMock.mockReturnValue({ series: 'Series X' });
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		const fileList: FileListInfo = {
			files: [
				{
					path: '/a.mp3',
					size: 1,
					duration: 1,
					isValid: true,
					bitrate: 64,
					sampleRate: 44100,
					channels: 2,
				},
				{
					path: '/b.mp3',
					size: 1,
					duration: 1,
					isValid: true,
					bitrate: 64,
					sampleRate: 44100,
					channels: 2,
				},
			],
			selectedDecoders: [null, null],
			totalDuration: 2,
			totalSize: 2,
			validCount: 2,
			invalidCount: 0,
		};
		setCurrentFileList(fileList);
		setSelectedFileIndices([0, 1]);
	});

	it('stages changes across selected files', async () => {
		const didStage = await stageMetadataToSelection({ showStatus: false });
		expect(didStage).toBe(true);
		expect(getMetadataForFile('/a.mp3')).toMatchObject({ series: 'Series X' });
		expect(getMetadataForFile('/b.mp3')).toMatchObject({ series: 'Series X' });
		expect(getPendingMetadataEntries()).toEqual([
			['/a.mp3', { series: 'Series X' }],
			['/b.mp3', { series: 'Series X' }],
		]);
	});

	it('skips pending writes when staged metadata is nullish-equivalent to existing drafts', async () => {
		setMetadataForFile('/a.mp3', { series: 'Series X' });
		setMetadataForFile('/b.mp3', { series: 'Series X' });

		const didStage = await stageMetadataToSelection({ showStatus: false });

		expect(didStage).toBe(true);
		expect(getMetadataForFile('/a.mp3')).toMatchObject({ series: 'Series X' });
		expect(getMetadataForFile('/b.mp3')).toMatchObject({ series: 'Series X' });
		expect(getPendingMetadataEntries()).toEqual([]);
	});

	it('surfaces validation errors instead of staging invalid metadata', async () => {
		context.validationErrorMock.mockReturnValue('Series part must be a number');

		const didStage = await stageMetadataToSelection({ showStatus: false });

		expect(didStage).toBe(false);
		expect(getMetadataForFile('/a.mp3')).toBeUndefined();
		expect(getPendingMetadataEntries()).toEqual([]);
	});

	it('treats one valid selected file as single-selection pending metadata', async () => {
		context.selectedFilesMock.mockReturnValue([
			{ path: '/invalid.mp3', isValid: false },
			{ path: '/a.mp3', isValid: true },
		]);
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ title: 'Single Valid' });

		const didPersist = await persistPendingMetadataDraftsForCurrentSelection({
			showStatus: false,
		});

		expect(didPersist).toBe(true);
		expect(context.readMetadataFormMock).toHaveBeenCalledWith({ mode: 'single' });
		expect(getMetadataForFile('/a.mp3')).toMatchObject({ title: 'Single Valid' });
		expect(getMetadataForFile('/invalid.mp3')).toBeUndefined();
	});
});
