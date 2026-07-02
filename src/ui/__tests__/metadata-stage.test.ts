import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	persistPendingMetadataDraftsForCurrentSelection,
	stageMetadataToSelection,
} from '../fileList/metadataStaging';
import { setCurrentFileList, setSelectedFileIndices } from '../fileList/state.svelte';
import {
	cacheMetadataForFile,
	clearMetadataSession,
	collectActionableMetadataIntent,
	getMetadataForFile,
} from '../metadataSession';
import type { FileListInfo } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';

const context = vi.hoisted(() => ({
	// Boundary mock: echoes the compiled patch back as the normalized patch and
	// reports whatever field errors the test arms.
	fieldErrorsMock: vi.fn<() => Array<{ field: string; code: string; message: string }>>(() => []),
	validateMetadataIntentPatchMock: vi.fn(),
	selectedFilesMock: vi.fn(() => [
		{ path: '/a.mp3', isValid: true },
		{ path: '/b.mp3', isValid: true },
	]),
	readMetadataFormMock: vi.fn<() => Partial<AudiobookMetadata>>(() => ({ series: 'Series X' })),
	hasDirtyMetadataFieldsMock: vi.fn(() => false),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		validateMetadataIntentPatch: context.validateMetadataIntentPatchMock,
		saveMetadataBatch: vi.fn(),
		listen: vi.fn(async () => () => {}),
	},
}));

vi.mock('../fileList/metadataPanel', () => ({
	ensureMetadataForFiles: vi.fn(async () => undefined),
	getSelectedFiles: context.selectedFilesMock,
}));

vi.mock('../outputPanel', () => ({
	updateEstimatedSize: vi.fn(),
	updateOutputPath: vi.fn(),
}));

vi.mock('../statusPanel', () => ({
	initStatusPanel: vi.fn(),
	isStatusPanelProcessing: vi.fn(() => false),
	pushStatusPanelTransientStatus: vi.fn(),
}));

vi.mock('../metadataForm', () => ({
	readMetadataForm: context.readMetadataFormMock,
	hasDirtyMetadataFields: context.hasDirtyMetadataFieldsMock,
	resetDirtyState: vi.fn(),
}));

describe('stageMetadataToSelection', () => {
	beforeEach(() => {
		document.body.innerHTML = '';
		clearMetadataSession();
		context.fieldErrorsMock.mockReset();
		context.fieldErrorsMock.mockReturnValue([]);
		context.validateMetadataIntentPatchMock.mockReset();
		context.validateMetadataIntentPatchMock.mockImplementation(async (patch: unknown) => {
			const fieldErrors = context.fieldErrorsMock();
			return {
				isValid: fieldErrors.length === 0,
				metadataPatch: patch,
				fieldErrors,
			};
		});
		context.selectedFilesMock.mockReset();
		context.readMetadataFormMock.mockReset();
		context.hasDirtyMetadataFieldsMock.mockReset();
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
		expect(collectActionableMetadataIntent(['/a.mp3', '/b.mp3'])).toEqual({
			'/a.mp3': { series: { op: 'set', value: 'Series X' } },
			'/b.mp3': { series: { op: 'set', value: 'Series X' } },
		});
	});

	it('skips pending writes when staged metadata is nullish-equivalent to existing drafts', async () => {
		cacheMetadataForFile('/a.mp3', { series: 'Series X' });
		cacheMetadataForFile('/b.mp3', { series: 'Series X' });

		const didStage = await stageMetadataToSelection({ showStatus: false });

		expect(didStage).toBe(true);
		expect(getMetadataForFile('/a.mp3')).toMatchObject({ series: 'Series X' });
		expect(getMetadataForFile('/b.mp3')).toMatchObject({ series: 'Series X' });
		expect(collectActionableMetadataIntent(['/a.mp3', '/b.mp3'])).toBeNull();
	});

	it('surfaces validation errors instead of staging invalid metadata', async () => {
		context.fieldErrorsMock.mockReturnValue([
			{
				field: 'series_part',
				code: 'series_part_contains_slash',
				message: 'Series part must be a number',
			},
		]);

		const didStage = await stageMetadataToSelection({ showStatus: false });

		expect(didStage).toBe(false);
		expect(getMetadataForFile('/a.mp3')).toBeUndefined();
		expect(collectActionableMetadataIntent(['/a.mp3', '/b.mp3'])).toBeNull();
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
