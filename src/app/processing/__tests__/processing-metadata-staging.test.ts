import { beforeEach, describe, expect, it, vi } from 'vitest';
import { defaultEncoderSettings, type ProcessPayload } from '../../../types/audio';
import type { MetadataIntentPatch } from '../../../types/metadataIntent';
import type { ProcessingStatus } from '../state';
import {
	startProcessing as startProcessingRaw,
	makeProcessingWorkflowServicesLayer,
} from '../workflow';
import type { ProcessingWorkflowServices } from '../workflow';
import { openGeneratedPreviewIfSingle } from '../preview';

const context = vi.hoisted(() => ({
	preflightProcessingPlanMock: vi.fn(),
	processAudiobookFilesMock: vi.fn(),
	submitProcessingOperationMock: vi.fn(),
	validateMetadataIntentPatchMock: vi.fn(async () => ({
		isValid: true,
		metadataPatch: {},
		fieldErrors: [],
	})),
	readAudioMetadataMock: vi.fn(),
	openPathMock: vi.fn(),
	getCurrentFileListMock: vi.fn(),
	getSelectedFileIndexMock: vi.fn(),
	getSelectedFileIndicesMock: vi.fn(),
	readProcessingRequestConfigMock: vi.fn(),
	runOutputPlanReviewWorkflowMock: vi.fn(),
	getJobTypeMock: vi.fn(),
	readMetadataFormMock: vi.fn(),
	hasDirtyMetadataFieldsMock: vi.fn(),
	// Session-strip fakes: stored patches live here; the collect fake applies
	// the real actionable/path filter so payload-level pins stay meaningful.
	storedIntentPatches: {} as Record<string, Record<string, { op: string; value?: unknown }>>,
	getMetadataForFileMock: vi.fn(),
	cacheMetadataForFileMock: vi.fn(),
	stageMetadataIntentPatchMock: vi.fn(() => 'staged' as const),
	stageMetadataToSelectionMock: vi.fn(),
	seriesPartValidationErrorMock: vi.fn(),
	subseriesPartValidationErrorMock: vi.fn(),
}));

vi.mock('../../../lib/tauri/client', () => ({
	tauriClient: {
		preflightProcessingPlan: context.preflightProcessingPlanMock,
		processAudiobookFiles: context.processAudiobookFilesMock,
		submitProcessingOperation: context.submitProcessingOperationMock,
		validateMetadataIntentPatch: context.validateMetadataIntentPatchMock,
		readAudioMetadata: context.readAudioMetadataMock,
		openPath: context.openPathMock,
	},
}));

vi.mock('../../metadataSession', () => ({
	getMetadataForFile: context.getMetadataForFileMock,
	cacheMetadataForFile: context.cacheMetadataForFileMock,
	stageMetadataIntentPatch: context.stageMetadataIntentPatchMock,
	collectActionableMetadataIntent: (filePaths: readonly string[]) => {
		const collected: Record<string, Record<string, { op: string }>> = {};
		for (const filePath of filePaths) {
			const patch = context.storedIntentPatches[filePath];
			if (patch && Object.values(patch).some((intent) => intent && intent.op !== 'noop')) {
				collected[filePath] = patch;
			}
		}
		return Object.keys(collected).length > 0 ? collected : null;
	},
	isUsableMetadataCache: (metadata: Record<string, unknown> | undefined) =>
		Boolean(
			metadata &&
				Object.entries(metadata).some(([key, value]) => value !== undefined && key !== 'cover_art'),
		),
	validateMetadataDraft: async (metadata: Record<string, unknown>) => {
		const first = context.seriesPartValidationErrorMock() ?? null;
		return {
			intentPatch: Object.fromEntries(
				Object.entries(metadata).map(([key, value]) => [
					key,
					(typeof value === 'string' && value.trim().length === 0) ||
					(Array.isArray(value) && value.length === 0)
						? { op: 'clear' }
						: { op: 'set', value },
				]),
			),
			ok: first == null,
			errors: { first, byField: {} },
			result: { isValid: first == null, metadataPatch: {}, fieldErrors: [] },
		};
	},
	metadataSaveInProgress: { subscribe: vi.fn() },
}));

const showError = vi.fn();

function processingContext() {
	return {
		updateStatus: vi.fn((_status: ProcessingStatus) => undefined),
		setProcessingState: vi.fn(),
		updateArtThumbnail: vi.fn(async () => undefined),
		startProgressListener: vi.fn(async () => undefined),
		setCurrentWorkKind: vi.fn(),
		setBatchCompletionMessage: vi.fn(),
		handleCancellation: vi.fn(),
		resetToIdle: vi.fn(),
	};
}

function collectStoredIntent(filePaths: readonly string[]) {
	const collected: Record<string, Record<string, { op: string }>> = {};
	for (const filePath of filePaths) {
		const patch = context.storedIntentPatches[filePath];
		if (patch && Object.values(patch).some((intent) => intent && intent.op !== 'noop')) {
			collected[filePath] = patch;
		}
	}
	return Object.keys(collected).length > 0 ? collected : null;
}

function stagingServices(): ProcessingWorkflowServices {
	return {
		getCurrentFileList: context.getCurrentFileListMock,
		getSelectedFileIndex: context.getSelectedFileIndexMock,
		getSelectedFileIndices: context.getSelectedFileIndicesMock,
		readProcessingRequestConfig: context.readProcessingRequestConfigMock,
		getJobType: context.getJobTypeMock,
		hasDirtyMetadataFields: context.hasDirtyMetadataFieldsMock,
		readMetadataForm: context.readMetadataFormMock,
		collectActionableMetadataIntent:
			collectStoredIntent as ProcessingWorkflowServices['collectActionableMetadataIntent'],
		getMetadataForFile: context.getMetadataForFileMock,
		cacheMetadataForFile: context.cacheMetadataForFileMock,
		stageMetadataIntentPatch: context.stageMetadataIntentPatchMock,
		stageMetadataToSelection: context.stageMetadataToSelectionMock,
		setJobControlsEnabled: vi.fn(),
		setFileOrderLocked: vi.fn(),
		validateMetadataIntentPatch: context.validateMetadataIntentPatchMock,
		readAudioMetadata: context.readAudioMetadataMock,
		processAudiobookFiles: context.processAudiobookFilesMock,
		submitProcessingOperation: context.submitProcessingOperationMock,
		remoteSource: {
			processingAssets: vi.fn(() => undefined),
			withSubmissionRetention: vi.fn(async (_inputIds, submit) => submit()),
		},
		runOutputPlanReviewWorkflow: context.runOutputPlanReviewWorkflowMock,
		openGeneratedPreviewIfSingle,
		feedback: { showError },
		console,
	};
}

function startProcessing(
	ctx: ReturnType<typeof processingContext>,
	options?: { previewSeconds?: number },
) {
	return startProcessingRaw(ctx, options, makeProcessingWorkflowServicesLayer(stagingServices()));
}

describe('startProcessing metadata staging', () => {
	beforeEach(() => {
		context.preflightProcessingPlanMock.mockReset();
		context.processAudiobookFilesMock.mockReset();
		context.submitProcessingOperationMock.mockReset();
		context.validateMetadataIntentPatchMock.mockClear();
		context.readAudioMetadataMock.mockReset();
		context.openPathMock.mockReset();
		context.getCurrentFileListMock.mockReset();
		context.getSelectedFileIndexMock.mockReset();
		context.getSelectedFileIndicesMock.mockReset();
		context.readProcessingRequestConfigMock.mockReset();
		context.runOutputPlanReviewWorkflowMock.mockReset();
		context.getJobTypeMock.mockReset();
		context.readMetadataFormMock.mockReset();
		context.hasDirtyMetadataFieldsMock.mockReset();
		context.storedIntentPatches = {};
		context.getMetadataForFileMock.mockReset();
		context.cacheMetadataForFileMock.mockReset();
		context.stageMetadataIntentPatchMock.mockReset();
		context.stageMetadataIntentPatchMock.mockReturnValue('staged');
		context.stageMetadataToSelectionMock.mockReset();
		context.seriesPartValidationErrorMock.mockReset();
		context.subseriesPartValidationErrorMock.mockReset();
		showError.mockReset();

		context.getCurrentFileListMock.mockReturnValue({
			files: [
				{ path: '/books/a.m4b', isValid: true },
				{ path: '/books/b.m4b', isValid: true },
			],
			validCount: 2,
		});
		context.getSelectedFileIndexMock.mockReturnValue(0);
		context.getSelectedFileIndicesMock.mockReturnValue(new Set([0]));
		context.readProcessingRequestConfigMock.mockReturnValue({
			encoderSettings: defaultEncoderSettings(),
			sampleRate: 'auto',
			outputDirectory: '/tmp/out',
			outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
		});
		context.getJobTypeMock.mockReturnValue('merge');
		context.getMetadataForFileMock.mockReturnValue(undefined);
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
			summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
			results: [{ inputIndex: 0, status: 'success', message: 'ok', jobId: 'job-1' }],
		});
		context.runOutputPlanReviewWorkflowMock.mockImplementation(
			async ({
				payload,
				metadataIntentByPath,
				previewSeconds,
			}: {
				payload: ProcessPayload;
				metadataIntentByPath: Record<string, MetadataIntentPatch> | null;
				previewSeconds?: number;
			}) => ({
				status: 'approved',
				payload: { ...payload, preflightSignature: 'preflight-approved' },
				plan: await context.preflightProcessingPlanMock({
					payload,
					metadataIntentByPath,
					previewSeconds,
				}),
			}),
		);
		context.submitProcessingOperationMock.mockResolvedValue({
			operationId: 'operation-1',
			snapshot: {
				operationId: 'operation-1',
				sequence: 1,
				kind: 'processingMerge',
				status: 'accepted',
				title: 'Merge encode (2 files)',
				createdAtMs: 1,
				startedAtMs: undefined,
				finishedAtMs: undefined,
				cancellable: true,
				cancelRequested: false,
				lanes: ['analysis', 'encodeCpu', 'outputCommit'],
				sourceInputIds: ['input-1', 'input-2'],
				progress: {
					stage: 'pending',
					percentage: 0,
					message: 'Accepted.',
					currentItemIndex: undefined,
					totalItems: 1,
					bytesDownloaded: undefined,
					bytesTotal: undefined,
					etaSeconds: undefined,
				},
				children: [],
				terminalSummary: undefined,
				warnings: [],
				errors: [],
			},
		});
		context.readAudioMetadataMock.mockResolvedValue({});
		context.seriesPartValidationErrorMock.mockReturnValue(null);
		context.subseriesPartValidationErrorMock.mockReturnValue(null);
	});

	it('loads batch metadata when cache only contains cover art', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.getMetadataForFileMock.mockImplementation((filePath: string) => {
			if (filePath === '/books/a.m4b') {
				return { cover_art: [1, 2, 3] };
			}
			if (filePath === '/books/b.m4b') {
				return { title: 'Already Loaded' };
			}
			return undefined;
		});
		context.readAudioMetadataMock.mockResolvedValue({ title: 'Loaded From Disk' });

		await startProcessing(processingContext());

		expect(context.readAudioMetadataMock).toHaveBeenCalledTimes(1);
		expect(context.readAudioMetadataMock).toHaveBeenCalledWith('/books/a.m4b');
		expect(context.cacheMetadataForFileMock).toHaveBeenCalledWith('/books/a.m4b', {
			title: 'Loaded From Disk',
		});
	});

	it('does not snapshot empty metadata when no dirty form edits exist', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.readMetadataFormMock.mockReturnValue({ title: '' });

		await startProcessing(processingContext());

		expect(context.readMetadataFormMock).not.toHaveBeenCalled();
		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
		expect(context.submitProcessingOperationMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: null,
			}),
		);
	});

	it('stages current-file metadata only when dirty edits exist', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ title: 'Edited Title' });
		context.storedIntentPatches = {
			'/books/a.m4b': { title: { op: 'set', value: 'Edited Title' } },
		};
		context.getMetadataForFileMock.mockReturnValue({ title: 'Edited Title' });

		await startProcessing(processingContext());

		expect(context.readMetadataFormMock).toHaveBeenCalledWith({ mode: 'single' });
		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith('/books/a.m4b', {
			title: { op: 'set', value: 'Edited Title' },
		});
		expect(context.submitProcessingOperationMock).toHaveBeenCalledWith(
			expect.objectContaining({
				payload: expect.objectContaining({
					preflightSignature: 'preflight-approved',
				}),
				metadataIntent: {
					'/books/a.m4b': { title: { op: 'set', value: 'Edited Title' } },
				},
			}),
		);
	});

	it('aborts processing when multi-selection staging fails', async () => {
		context.getSelectedFileIndicesMock.mockReturnValue(new Set([0, 1]));
		context.stageMetadataToSelectionMock.mockResolvedValue(false);

		await startProcessing(processingContext());

		expect(context.stageMetadataToSelectionMock).toHaveBeenCalledWith({
			showStatus: false,
		});
		expect(context.submitProcessingOperationMock).not.toHaveBeenCalled();
		expect(showError).toHaveBeenCalledWith(
			'Fix metadata validation errors before processing.',
		);
	});

	it('aborts single-selection processing when dirty series-part metadata is invalid', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ series_part: '1/2' });
		context.seriesPartValidationErrorMock.mockReturnValue('Series part must be a number');

		await startProcessing(processingContext());

		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
		expect(context.submitProcessingOperationMock).not.toHaveBeenCalled();
		expect(showError).toHaveBeenCalledWith('Series part must be a number');
	});

	it('aborts non-merge processing instead of retargeting dirty edits from an invalid row', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.getCurrentFileListMock.mockReturnValue({
			files: [
				{ path: '/books/invalid.m4b', isValid: false },
				{ path: '/books/valid.m4b', isValid: true },
			],
			validCount: 1,
		});
		context.getSelectedFileIndexMock.mockReturnValue(0);
		context.getSelectedFileIndicesMock.mockReturnValue(new Set([0]));
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ title: 'Should Not Retarget' });
		vi.mocked(showError).mockClear();

		await startProcessing(processingContext());

		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalled();
		expect(context.submitProcessingOperationMock).not.toHaveBeenCalled();
		expect(showError).toHaveBeenCalledWith(
			'Select a valid input file before processing metadata edits.',
		);
	});

	it('stages clear intent for dirty-but-empty metadata in merge payload', async () => {
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({ title: '   ' });
		context.getMetadataForFileMock.mockReturnValue({ title: '' });
		context.storedIntentPatches = { '/books/a.m4b': { title: { op: 'clear' } } };

		await startProcessing(processingContext());

		expect(context.readMetadataFormMock).toHaveBeenCalledWith({ mode: 'single' });
		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith('/books/a.m4b', {
			title: { op: 'clear' },
		});
		expect(context.submitProcessingOperationMock).toHaveBeenCalledWith(
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
		context.storedIntentPatches = { '/books/a.m4b': { cover_art: { op: 'clear' } } };

		await startProcessing(processingContext());

		expect(context.readMetadataFormMock).toHaveBeenCalledWith({ mode: 'single' });
		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith('/books/a.m4b', {
			cover_art: { op: 'clear' },
		});
		expect(context.submitProcessingOperationMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: {
					'/books/a.m4b': { cover_art: { op: 'clear' } },
				},
			}),
		);
	});

	it('stages merge metadata intent onto the merge key when a non-first file is selected', async () => {
		context.getSelectedFileIndexMock.mockReturnValue(1);
		context.getSelectedFileIndicesMock.mockReturnValue(new Set([1]));
		context.hasDirtyMetadataFieldsMock.mockReturnValue(true);
		context.readMetadataFormMock.mockReturnValue({
			title: 'Selected Row Title',
			cover_art: [7, 7, 7],
		});
		context.getMetadataForFileMock.mockReturnValue({});
		context.storedIntentPatches = {
			'/books/a.m4b': {
				title: { op: 'set', value: 'Selected Row Title' },
				cover_art: { op: 'set', value: [7, 7, 7] },
			},
		};

		await startProcessing(processingContext());

		expect(context.stageMetadataIntentPatchMock).toHaveBeenCalledWith('/books/a.m4b', {
			title: { op: 'set', value: 'Selected Row Title' },
			cover_art: { op: 'set', value: [7, 7, 7] },
		});
		expect(context.stageMetadataIntentPatchMock).not.toHaveBeenCalledWith(
			'/books/b.m4b',
			expect.anything(),
		);
		expect(context.runOutputPlanReviewWorkflowMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntentByPath: {
					'/books/a.m4b': {
						title: { op: 'set', value: 'Selected Row Title' },
						cover_art: { op: 'set', value: [7, 7, 7] },
					},
				},
			}),
		);
		expect(context.submitProcessingOperationMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: {
					'/books/a.m4b': {
						title: { op: 'set', value: 'Selected Row Title' },
						cover_art: { op: 'set', value: [7, 7, 7] },
					},
				},
			}),
		);
	});

	it('keeps batch metadata intent entries, including clear-intent values', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.storedIntentPatches = {
			'/books/a.m4b': { title: { op: 'clear' } },
			'/books/b.m4b': { series: { op: 'set', value: 'Series B' } },
		};

		await startProcessing(processingContext());

		expect(context.submitProcessingOperationMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: {
					'/books/a.m4b': { title: { op: 'clear' } },
					'/books/b.m4b': { series: { op: 'set', value: 'Series B' } },
				},
			}),
		);
	});

	it('does not build a synchronous batch failure summary after background submission', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.storedIntentPatches = {};
		context.processAudiobookFilesMock.mockResolvedValue({
			jobType: 'batch',
			summary: { total: 2, succeeded: 0, skipped: 0, cancelled: 0, failed: 1 },
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

		expect(ctx.setBatchCompletionMessage).toHaveBeenCalledWith(null);
	});

	it('does not build a synchronous mixed batch summary after background submission', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.storedIntentPatches = {};
		context.processAudiobookFilesMock.mockResolvedValue({
			jobType: 'batch',
			summary: { total: 2, succeeded: 1, skipped: 0, cancelled: 1, failed: 0 },
			results: [
				{
					inputIndex: 0,
					status: 'success',
					message: 'Successfully created audiobook: /tmp/out/a.m4b',
					jobId: 'job-1',
					error: null,
					previewFilePath: null,
					previewActualSeconds: null,
				},
				{
					inputIndex: 1,
					status: 'cancelled',
					message: 'Processing was cancelled',
					jobId: 'job-2',
					error: {
						code: 'processing_cancelled',
						category: 'cancellation',
						message: 'Processing was cancelled',
						detail: null,
					},
					previewFilePath: null,
					previewActualSeconds: null,
				},
			],
		});

		const ctx = processingContext();

		await startProcessing(ctx);

		expect(ctx.setBatchCompletionMessage).toHaveBeenCalledWith(null);
	});

	it('treats structured cancellation errors as cancellation instead of failures', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.storedIntentPatches = {};
		context.submitProcessingOperationMock.mockRejectedValueOnce({
			code: 'cancelled',
			category: 'cancellation',
			message: 'Processing was cancelled.',
			detail: 'user requested stop',
		});

		const ctx = processingContext();
		vi.mocked(showError).mockClear();

		await startProcessing(ctx);

		expect(showError).not.toHaveBeenCalled();
		expect(ctx.handleCancellation).toHaveBeenCalledTimes(1);
		expect(ctx.resetToIdle).not.toHaveBeenCalled();
	});

	it('filters batch metadata intent to active input files only', async () => {
		context.getJobTypeMock.mockReturnValue('batch');
		context.hasDirtyMetadataFieldsMock.mockReturnValue(false);
		context.getMetadataForFileMock.mockReturnValue({ title: 'Already Loaded' });
		context.storedIntentPatches = {
			'/books/a.m4b': { title: { op: 'set', value: 'Active A' } },
			'/books/b.m4b': { series: { op: 'set', value: 'Active B' } },
			'/books/stale.m4b': { title: { op: 'set', value: 'Stale' } },
		};

		await startProcessing(processingContext());

		expect(context.submitProcessingOperationMock).toHaveBeenCalledWith(
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
		context.storedIntentPatches = {
			'/books/a.m4b': {},
			'/books/b.m4b': {},
		};

		await startProcessing(processingContext());

		expect(context.submitProcessingOperationMock).toHaveBeenCalledWith(
			expect.objectContaining({
				metadataIntent: null,
			}),
		);
	});

	it('auto-opens preview only when exactly one successful preview path is returned', async () => {
		context.processAudiobookFilesMock.mockResolvedValue({
			jobType: 'merge',
			summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
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

		expect(context.openPathMock).toHaveBeenCalledTimes(1);
		expect(context.openPathMock).toHaveBeenCalledWith('/tmp/out/one.preview.m4b');
	});

	it('does not auto-open preview when multiple successful preview paths are returned', async () => {
		context.processAudiobookFilesMock.mockResolvedValue({
			jobType: 'batch',
			summary: { total: 2, succeeded: 2, skipped: 0, cancelled: 0, failed: 0 },
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

		expect(context.openPathMock).not.toHaveBeenCalled();
	});

	it('does not auto-open preview for failed result entries', async () => {
		context.processAudiobookFilesMock.mockResolvedValue({
			jobType: 'merge',
			summary: { total: 1, succeeded: 0, skipped: 0, cancelled: 0, failed: 1 },
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

		expect(context.openPathMock).not.toHaveBeenCalled();
	});
});
