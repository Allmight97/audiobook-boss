import { beforeEach, describe, expect, it, vi } from 'vitest';
import { defaultEncoderSettings } from '../../../types/audio';
import type { ProcessingStatus } from '../state';
import { startProcessing } from '../processing';
import * as dom from '../dom';

const context = vi.hoisted(() => ({
	preflightProcessingPlanMock: vi.fn(),
	processAudiobookFilesMock: vi.fn(),
	readAudioMetadataMock: vi.fn(),
	openExternalMock: vi.fn(),
	getCurrentFileListMock: vi.fn(),
	getSelectedFileIndexMock: vi.fn(),
	getSelectedFileIndicesMock: vi.fn(),
	readOutputConfigForProcessingMock: vi.fn(),
	updateOutputPathMock: vi.fn(),
	getJobTypeMock: vi.fn(),
	readMetadataFormMock: vi.fn(),
	hasDirtyMetadataFieldsMock: vi.fn(),
	getAllMetadataIntentPatchesMock: vi.fn(),
	getMetadataForFileMock: vi.fn(),
	getMetadataIntentPatchForFileMock: vi.fn(),
	setMetadataForFileMock: vi.fn(),
	stageMetadataToSelectionMock: vi.fn(),
}));

vi.mock('../../../lib/tauri/client', () => ({
	tauriClient: {
		preflightProcessingPlan: context.preflightProcessingPlanMock,
		processAudiobookFiles: context.processAudiobookFilesMock,
		readAudioMetadata: context.readAudioMetadataMock,
		openExternal: context.openExternalMock,
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
	readOutputConfigForProcessing: context.readOutputConfigForProcessingMock,
	updateOutputPath: context.updateOutputPathMock,
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
	getAllMetadataIntentPatches: context.getAllMetadataIntentPatchesMock,
	getMetadataForFile: context.getMetadataForFileMock,
	getMetadataIntentPatchForFile: context.getMetadataIntentPatchForFileMock,
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
		setBatchCompletionMessage: vi.fn(),
		handleCancellation: vi.fn(),
		resetToIdle: vi.fn(),
	};
}

describe('startProcessing metadata staging', () => {
	beforeEach(() => {
		context.preflightProcessingPlanMock.mockReset();
		context.processAudiobookFilesMock.mockReset();
		context.readAudioMetadataMock.mockReset();
		context.openExternalMock.mockReset();
		context.getCurrentFileListMock.mockReset();
		context.getSelectedFileIndexMock.mockReset();
		context.getSelectedFileIndicesMock.mockReset();
		context.readOutputConfigForProcessingMock.mockReset();
		context.updateOutputPathMock.mockReset();
		context.getJobTypeMock.mockReset();
		context.readMetadataFormMock.mockReset();
		context.hasDirtyMetadataFieldsMock.mockReset();
		context.getAllMetadataIntentPatchesMock.mockReset();
		context.getMetadataForFileMock.mockReset();
		context.getMetadataIntentPatchForFileMock.mockReset();
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
		context.readOutputConfigForProcessingMock.mockReturnValue({
			encoderSettings: defaultEncoderSettings(),
			toolchainSettings: {},
			sampleRate: 'auto',
			outputPath: '/tmp/out',
			outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
		});
		context.getJobTypeMock.mockReturnValue('merge');
		context.getAllMetadataIntentPatchesMock.mockReturnValue({});
		context.getMetadataForFileMock.mockReturnValue(undefined);
		context.getMetadataIntentPatchForFileMock.mockReturnValue(undefined);
		context.preflightProcessingPlanMock.mockImplementation(async ({ payload, previewSeconds }) => ({
			jobType: payload.jobType ?? 'merge',
			previewSeconds: previewSeconds ?? undefined,
			collisionPolicy: payload.collisionPolicy ?? 'fail',
			planSignature: 'preflight-clean',
			outputs: (payload.inputFiles ?? []).map((filePath: string, index: number) => ({
				inputIndex: index,
				inputPath: filePath,
				kind: previewSeconds == null ? 'final' : 'preview',
				requestedPath: `/tmp/out/${index}.m4b`,
				resolvedPath: `/tmp/out/${index}.m4b`,
				renameCandidate: undefined,
				collision: undefined,
				action: 'write',
			})),
		}));
		context.processAudiobookFilesMock.mockResolvedValue({
			jobType: 'merge',
			summary: { total: 1, succeeded: 1, skipped: 0, failed: 0 },
			results: [{ inputIndex: 0, status: 'success', message: 'ok', jobId: 'job-1' }],
		});
		context.readAudioMetadataMock.mockResolvedValue({});
	});

	it('does not snapshot empty metadata when no dirty form edits exist', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.readMetadataFormMock.mockReturnValue({ title: '' });

		await startProcessing(processingContext());

		expect(context.readMetadataFormMock).not.toHaveBeenCalled();
		expect(context.setMetadataForFileMock).not.toHaveBeenCalled();
		expect(context.processAudiobookFilesMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: null,
			}),
		);
	});

	it('stages current-file metadata only when dirty edits exist', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ title: 'Edited Title' });
		context.getMetadataIntentPatchForFileMock.mockReturnValue({
			title: { op: 'set', value: 'Edited Title' },
		});
		context.getMetadataForFileMock.mockReturnValue({ title: 'Edited Title' });

		await startProcessing(processingContext());

		expect(context.readMetadataFormMock).toHaveBeenCalledWith({ mode: 'single' });
		expect(context.setMetadataForFileMock).toHaveBeenCalledWith(
			'/books/a.m4b',
			{
				title: 'Edited Title',
			},
			{
				intentPatch: { title: { op: 'set', value: 'Edited Title' } },
				markPending: true,
			},
		);
		expect(context.processAudiobookFilesMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: {
					'/books/a.m4b': { title: { op: 'set', value: 'Edited Title' } },
				},
			}),
		);
	});

	it('stages clear intent for dirty-but-empty metadata in merge payload', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ title: '   ' });
		context.getMetadataForFileMock.mockReturnValue({ title: '' });
		context.getMetadataIntentPatchForFileMock.mockReturnValue({ title: { op: 'clear' } });

		await startProcessing(processingContext());

		expect(context.readMetadataFormMock).toHaveBeenCalledWith({ mode: 'single' });
		expect(context.setMetadataForFileMock).toHaveBeenCalledWith(
			'/books/a.m4b',
			{},
			{
				intentPatch: { title: { op: 'clear' } },
				markPending: true,
			},
		);
		expect(context.processAudiobookFilesMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: {
					'/books/a.m4b': { title: { op: 'clear' } },
				},
			}),
		);
	});

	it('stages cover-art clear intent even when no text fields are dirty', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ cover_art: [] });
		context.getMetadataForFileMock.mockReturnValue({});
		context.getMetadataIntentPatchForFileMock.mockReturnValue({ cover_art: { op: 'clear' } });

		await startProcessing(processingContext());

		expect(context.readMetadataFormMock).toHaveBeenCalledWith({ mode: 'single' });
		expect(context.setMetadataForFileMock).toHaveBeenCalledWith(
			'/books/a.m4b',
			{},
			{
				intentPatch: { cover_art: { op: 'clear' } },
				markPending: true,
			},
		);
		expect(context.processAudiobookFilesMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: {
					'/books/a.m4b': { cover_art: { op: 'clear' } },
				},
			}),
		);
	});

	it('keeps batch metadata intent entries, including clear-intent values', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.getAllMetadataIntentPatchesMock.mockReturnValue({
			'/books/a.m4b': { title: { op: 'clear' } },
			'/books/b.m4b': { series: { op: 'set', value: 'Series B' } },
		});

		await startProcessing(processingContext());

		expect(context.processAudiobookFilesMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: {
					'/books/a.m4b': { title: { op: 'clear' } },
					'/books/b.m4b': { series: { op: 'set', value: 'Series B' } },
				},
			}),
		);
	});

	it('summarizes structured batch failures from error envelopes', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.getAllMetadataIntentPatchesMock.mockReturnValue({});
		context.processAudiobookFilesMock.mockResolvedValue({
			jobType: 'batch',
			summary: { total: 2, succeeded: 0, failed: 1 },
			results: [
				{
					inputIndex: null,
					status: 'failed',
					message: 'failed',
					jobId: null,
					error: {
						code: 'decoder_unavailable',
						category: 'toolchain',
						message: 'decoder unavailable',
						detail: 'ffmpeg missing',
					},
					previewFilePath: null,
					previewActualSeconds: null,
				},
			],
		});

		const ctx = processingContext();

		await startProcessing(ctx);

		expect(ctx.setBatchCompletionMessage).toHaveBeenCalledWith(
			'No files were processed successfully. Failed: decoder unavailable',
		);
	});

	it('treats structured cancellation errors as cancellation instead of failures', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.getAllMetadataIntentPatchesMock.mockReturnValue({});
		context.processAudiobookFilesMock.mockRejectedValueOnce({
			code: 'cancelled',
			category: 'cancellation',
			message: 'Processing was cancelled.',
			detail: 'user requested stop',
		});

		const ctx = processingContext();
		vi.mocked(dom.showError).mockClear();

		await startProcessing(ctx);

		expect(dom.showError).not.toHaveBeenCalled();
		expect(ctx.handleCancellation).toHaveBeenCalledTimes(1);
		expect(ctx.resetToIdle).not.toHaveBeenCalled();
	});

	it('filters batch metadata intent to active input files only', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.getAllMetadataIntentPatchesMock.mockReturnValue({
			'/books/a.m4b': { title: { op: 'set', value: 'Active A' } },
			'/books/b.m4b': { series: { op: 'set', value: 'Active B' } },
			'/books/stale.m4b': { title: { op: 'set', value: 'Stale' } },
		});

		await startProcessing(processingContext());

		expect(context.processAudiobookFilesMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: {
					'/books/a.m4b': { title: { op: 'set', value: 'Active A' } },
					'/books/b.m4b': { series: { op: 'set', value: 'Active B' } },
				},
			}),
		);
	});

	it('sends null batch metadata intent when all stored entries are empty objects', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.getAllMetadataIntentPatchesMock.mockReturnValue({
			'/books/a.m4b': {},
			'/books/b.m4b': {},
		});

		await startProcessing(processingContext());

		expect(context.processAudiobookFilesMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: null,
			}),
		);
	});

	it('auto-opens preview only when exactly one successful preview path is returned', async () => {
		context.processAudiobookFilesMock.mockResolvedValue({
			jobType: 'merge',
			summary: { total: 1, succeeded: 1, failed: 0 },
			results: [
				{
					inputIndex: 0,
					status: 'success',
					message: 'preview ok',
					jobId: 'job-1',
					previewFilePath: '/tmp/out/one.preview.m4b',
					previewActualSeconds: 30,
				},
			],
		});

		await startProcessing(processingContext(), { previewSeconds: 30 });

		expect(context.openExternalMock).toHaveBeenCalledTimes(1);
		expect(context.openExternalMock).toHaveBeenCalledWith('/tmp/out/one.preview.m4b');
		expect(context.updateOutputPathMock).toHaveBeenNthCalledWith(1, 'preview');
		expect(context.updateOutputPathMock).toHaveBeenLastCalledWith('final');
	});

	it('does not auto-open preview when multiple successful preview paths are returned', async () => {
		context.processAudiobookFilesMock.mockResolvedValue({
			jobType: 'batch',
			summary: { total: 2, succeeded: 2, failed: 0 },
			results: [
				{
					inputIndex: 0,
					status: 'success',
					message: 'preview a',
					jobId: 'job-1',
					previewFilePath: '/tmp/out/a.preview.m4b',
				},
				{
					inputIndex: 1,
					status: 'success',
					message: 'preview b',
					jobId: 'job-2',
					previewFilePath: '/tmp/out/b.preview.m4b',
				},
			],
		});

		await startProcessing(processingContext(), { previewSeconds: 30 });

		expect(context.openExternalMock).not.toHaveBeenCalled();
	});

	it('does not auto-open preview for failed result entries', async () => {
		context.processAudiobookFilesMock.mockResolvedValue({
			jobType: 'merge',
			summary: { total: 1, succeeded: 0, failed: 1 },
			results: [
				{
					inputIndex: 0,
					status: 'failed',
					message: 'preview failed',
					error: 'decoder error',
					previewFilePath: '/tmp/out/failed.preview.m4b',
					previewActualSeconds: 30,
				},
			],
		});

		await startProcessing(processingContext(), { previewSeconds: 30 });

		expect(context.openExternalMock).not.toHaveBeenCalled();
	});
});
