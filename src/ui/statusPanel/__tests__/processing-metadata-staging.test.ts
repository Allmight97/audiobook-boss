import { beforeEach, describe, expect, it, vi } from 'vitest';
import { defaultEncoderSettings } from '../../../types/audio';
import type { ProcessingStatus } from '../state';
import { startProcessing } from '../processing';

const context = vi.hoisted(() => ({
	processAudiobookFilesV2Mock: vi.fn(),
	readAudioMetadataMock: vi.fn(),
	getCurrentFileListMock: vi.fn(),
	getSelectedFileIndexMock: vi.fn(),
	getSelectedFileIndicesMock: vi.fn(),
	getCurrentOutputConfigMock: vi.fn(),
	getJobTypeMock: vi.fn(),
	readMetadataFormMock: vi.fn(),
	hasDirtyMetadataFieldsMock: vi.fn(),
	getAllMetadataMock: vi.fn(),
	getMetadataForFileMock: vi.fn(),
	hasMeaningfulMetadataMock: vi.fn((metadata: Record<string, unknown>) =>
		Object.entries(metadata).some(([key, value]) =>
			Array.isArray(value)
				? key === 'cover_art' || value.length > 0
				: value != null && String(value).trim().length > 0,
		),
	),
	setMetadataForFileMock: vi.fn(),
	stageMetadataToSelectionMock: vi.fn(),
}));

vi.mock('../../../lib/bridge', () => ({
	bridge: {
		processAudiobookFilesV2: context.processAudiobookFilesV2Mock,
		readAudioMetadata: context.readAudioMetadataMock,
	},
}));

vi.mock('../../fileList', () => ({
	getCurrentFileList: context.getCurrentFileListMock,
	getSelectedFileIndex: context.getSelectedFileIndexMock,
	setFileOrderLocked: vi.fn(),
}));

vi.mock('../../fileList/state', () => ({
	getSelectedFileIndices: context.getSelectedFileIndicesMock,
}));

vi.mock('../../outputPanel', () => ({
	getCurrentOutputConfig: context.getCurrentOutputConfigMock,
}));

vi.mock('../../jobControls', () => ({
	getJobType: context.getJobTypeMock,
	setJobControlsEnabled: vi.fn(),
}));

vi.mock('../../metadataForm', () => ({
	readMetadataForm: context.readMetadataFormMock,
	hasDirtyMetadataFields: context.hasDirtyMetadataFieldsMock,
}));

vi.mock('../../metadataState', () => ({
	getAllMetadata: context.getAllMetadataMock,
	getMetadataForFile: context.getMetadataForFileMock,
	hasMeaningfulMetadata: context.hasMeaningfulMetadataMock,
	setMetadataForFile: context.setMetadataForFileMock,
}));

vi.mock('../../fileList/actions', () => ({
	stageMetadataToSelection: context.stageMetadataToSelectionMock,
}));

vi.mock('../dom', () => ({
	showError: vi.fn(),
}));

function processingContext() {
	return {
		updateStatus: vi.fn((_status: ProcessingStatus) => undefined),
		setProcessingState: vi.fn(),
		updateArtThumbnail: vi.fn(async () => undefined),
		startProgressListener: vi.fn(async () => undefined),
		setCurrentJobType: vi.fn(),
		resetToIdle: vi.fn(),
	};
}

describe('startProcessing metadata staging', () => {
	beforeEach(() => {
		context.processAudiobookFilesV2Mock.mockReset();
		context.readAudioMetadataMock.mockReset();
		context.getCurrentFileListMock.mockReset();
		context.getSelectedFileIndexMock.mockReset();
		context.getSelectedFileIndicesMock.mockReset();
		context.getCurrentOutputConfigMock.mockReset();
		context.getJobTypeMock.mockReset();
		context.readMetadataFormMock.mockReset();
		context.hasDirtyMetadataFieldsMock.mockReset();
		context.getAllMetadataMock.mockReset();
		context.getMetadataForFileMock.mockReset();
		context.hasMeaningfulMetadataMock.mockReset();
		context.setMetadataForFileMock.mockReset();
		context.stageMetadataToSelectionMock.mockReset();

		context.getCurrentFileListMock.mockReturnValue({
			files: [
				{ path: '/books/a.m4b', isValid: true },
				{ path: '/books/b.m4b', isValid: true },
			],
			validCount: 2,
		});
		context.getSelectedFileIndexMock.mockReturnValue(0);
		context.getSelectedFileIndicesMock.mockReturnValue(new Set([0]));
		context.getCurrentOutputConfigMock.mockReturnValue({
			encoderSettings: defaultEncoderSettings(),
			sampleRate: 'auto',
			outputPath: '/tmp/out',
			outputNaming: { absCompatible: true, includeYear: false },
		});
		context.getJobTypeMock.mockReturnValue('merge');
		context.getAllMetadataMock.mockReturnValue({});
		context.getMetadataForFileMock.mockReturnValue(undefined);
		context.hasMeaningfulMetadataMock.mockImplementation((metadata: Record<string, unknown>) =>
			Object.entries(metadata).some(([key, value]) =>
				Array.isArray(value)
					? key === 'cover_art' || value.length > 0
					: value != null && String(value).trim().length > 0,
			),
		);
		context.processAudiobookFilesV2Mock.mockResolvedValue({
			message: 'ok',
			jobId: 'job-1',
		});
		context.readAudioMetadataMock.mockResolvedValue({});
	});

	it('does not snapshot empty metadata when no dirty form edits exist', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.readMetadataFormMock.mockReturnValue({ title: '' });

		await startProcessing(processingContext());

		expect(context.readMetadataFormMock).not.toHaveBeenCalled();
		expect(context.setMetadataForFileMock).not.toHaveBeenCalled();
		expect(context.processAudiobookFilesV2Mock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadata: null,
			}),
		);
	});

	it('stages current-file metadata only when dirty edits exist', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ title: 'Edited Title' });

		await startProcessing(processingContext());

		expect(context.readMetadataFormMock).toHaveBeenCalledWith({ mode: 'single' });
		expect(context.setMetadataForFileMock).toHaveBeenCalledWith('/books/a.m4b', {
			title: 'Edited Title',
		});
		expect(context.processAudiobookFilesV2Mock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadata: {
					'/books/a.m4b': { title: 'Edited Title' },
				},
			}),
		);
	});

	it('does not stage dirty-but-empty metadata for merge payload', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ title: '   ' });

		await startProcessing(processingContext());

		expect(context.readMetadataFormMock).toHaveBeenCalledWith({ mode: 'single' });
		expect(context.hasMeaningfulMetadataMock).toHaveBeenCalledWith({ title: '   ' });
		expect(context.setMetadataForFileMock).not.toHaveBeenCalled();
		expect(context.processAudiobookFilesV2Mock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadata: null,
			}),
		);
	});

	it('filters meaningless batch metadata entries before invoking processing', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.getAllMetadataMock.mockReturnValue({
			'/books/a.m4b': { title: '   ' },
			'/books/b.m4b': { series: 'Series B' },
		});

		await startProcessing(processingContext());

		expect(context.processAudiobookFilesV2Mock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadata: {
					'/books/b.m4b': { series: 'Series B' },
				},
			}),
		);
	});

	it('sends null batch metadata when all stored entries are meaningless', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.getAllMetadataMock.mockReturnValue({
			'/books/a.m4b': { title: '   ' },
			'/books/b.m4b': { title: null },
		});

		await startProcessing(processingContext());

		expect(context.processAudiobookFilesV2Mock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadata: null,
			}),
		);
	});
});
