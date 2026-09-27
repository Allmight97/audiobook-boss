import { titleAudioRequest } from '../../../test/fixtures/titleAudio';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ProcessPayload } from '../../../types/audio';
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
	openPathMock: vi.fn(),
	currentTitlesMock: vi.fn(),
	readProcessingRequestConfigMock: vi.fn(),
	runOutputPlanReviewWorkflowMock: vi.fn(),
	stageMetadataMock: vi.fn(),
	intentsForProcessMock: vi.fn(),
}));

vi.mock('../../../lib/tauri/client', () => ({
	tauriClient: {
		preflightProcessingPlan: context.preflightProcessingPlanMock,
		processAudiobookFiles: context.processAudiobookFilesMock,
		submitProcessingOperation: context.submitProcessingOperationMock,
		openPath: context.openPathMock,
	},
}));

const showError = vi.fn();

function processingContext() {
	return {
		updateStatus: vi.fn((_status: ProcessingStatus) => undefined),
		setProcessingState: vi.fn(),
		updateArtThumbnail: vi.fn(async () => undefined),
		startProgressListener: vi.fn(async () => undefined),
		setBatchCompletionMessage: vi.fn(),
		handleCancellation: vi.fn(),
		resetToIdle: vi.fn(),
	};
}

function stagingServices(): ProcessingWorkflowServices {
	return {
		currentTitles: context.currentTitlesMock,
		sourcesFor: (file) => [file],
		readProcessingRequestConfig: context.readProcessingRequestConfigMock,
		stageMetadata: context.stageMetadataMock,
		intentsForProcess:
			context.intentsForProcessMock as ProcessingWorkflowServices['intentsForProcess'],
		setJobControlsEnabled: vi.fn(),
		setFileOrderLocked: vi.fn(),
		processAudiobookFiles: context.processAudiobookFilesMock,
		submitProcessingOperation: context.submitProcessingOperationMock,
		runOutputPlanReviewWorkflow: context.runOutputPlanReviewWorkflowMock,
		openGeneratedPreviewIfSingle,
		feedback: { showError },
		console,
		remoteSource: {
			processingAssets: vi.fn(() => undefined),
			withSubmissionRetention: vi.fn(async (_inputIds, submit) => submit()),
		},
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
		context.openPathMock.mockReset();
		context.currentTitlesMock.mockReset();
		context.readProcessingRequestConfigMock.mockReset();
		context.runOutputPlanReviewWorkflowMock.mockReset();
		context.stageMetadataMock.mockReset();
		context.stageMetadataMock.mockResolvedValue({ status: 'staged' });
		context.intentsForProcessMock.mockReset();
		context.intentsForProcessMock.mockResolvedValue(null);
		showError.mockReset();

		context.currentTitlesMock.mockReturnValue([
			{ path: '/books/a.m4b', isValid: true },
			{ path: '/books/b.m4b', isValid: true },
		]);
		context.readProcessingRequestConfigMock.mockReturnValue({
			audioRequests: [titleAudioRequest()],
			outputDirectory: '/tmp/out',
			outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
		});
		context.preflightProcessingPlanMock.mockImplementation(async ({ payload, previewSeconds }) => ({
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
				revision: 1,
				createdRevision: 1,
				kind: 'processingBatch',
				status: 'accepted',
				title: 'a.m4b + 1 more',
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
	});

	it.each([
		{ outcome: { status: 'invalid', message: 'Series part must be a number' } },
		{ outcome: { status: 'stale' } },
		{ outcome: { status: 'noTarget' } },
	] as const)('does not submit when metadata staging is $outcome.status', async ({ outcome }) => {
		context.stageMetadataMock.mockResolvedValue(outcome);

		await startProcessing(processingContext());

		expect(context.intentsForProcessMock).not.toHaveBeenCalled();
		expect(context.submitProcessingOperationMock).not.toHaveBeenCalled();
		expect(showError).toHaveBeenCalledWith(
			{
				invalid: 'Series part must be a number',
				stale: 'Metadata changed while preparing to process. Start processing again.',
				noTarget: 'Select a valid input file before processing metadata edits.',
			}[outcome.status],
		);
	});

	it('submits the intents Metadata returns for the submitted inputs', async () => {
		const intents = {
			'/books/a.m4b': { title: { op: 'clear' as const } },
			'/books/b.m4b': { series: { op: 'set' as const, value: 'Series B' } },
		};
		context.intentsForProcessMock.mockResolvedValue(intents);

		await startProcessing(processingContext());

		expect(context.intentsForProcessMock).toHaveBeenCalledWith(['/books/a.m4b', '/books/b.m4b']);
		expect(context.submitProcessingOperationMock).toHaveBeenCalledWith(
			expect.objectContaining({ metadataIntent: intents }),
		);
	});

	it('treats structured cancellation errors as cancellation instead of failures', async () => {
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

	it('auto-opens preview only when exactly one successful preview path is returned', async () => {
		context.processAudiobookFilesMock.mockResolvedValue({
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
