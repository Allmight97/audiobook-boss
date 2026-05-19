import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	makeProcessingWorkflowServicesLayer,
	startProcessing,
	type ProcessingWorkflowContext,
	type ProcessingWorkflowServices,
} from '../processingWorkflow';
import {
	defaultEncoderSettings,
	type AudioFile,
	type FileListInfo,
	type ProcessCommandResult,
	type ProcessingPreflightPlan,
	type ProcessingRequestConfig,
	type ProcessPayload,
	type JobType,
} from '../../../types/audio';
import type { OutputPlanReviewResult } from '../../outputPanel/outputPlanWorkflow';

function audioFile(path: string): AudioFile {
	return {
		path,
		size: 1,
		duration: 1,
		format: 'm4b',
		bitrate: undefined,
		sampleRate: undefined,
		channels: undefined,
		codecLabel: undefined,
		selectedDecoder: undefined,
		isValid: true,
		error: undefined,
	};
}

function fileList(paths = ['/books/a.m4b']): FileListInfo {
	return {
		files: paths.map(audioFile),
		selectedDecoders: paths.map(() => null),
		totalDuration: paths.length,
		totalSize: paths.length,
		validCount: paths.length,
		invalidCount: 0,
	};
}

function processingConfig(): ProcessingRequestConfig {
	return {
		encoderSettings: defaultEncoderSettings(),
		toolchainSettings: {},
		sampleRate: 'auto',
		outputDirectory: '/tmp/out',
		outputNaming: { preset: 'absDefault', includeYear: false, customTemplate: undefined },
	};
}

function preflightPlan(payload: ProcessPayload): ProcessingPreflightPlan {
	return {
		jobType: payload.jobType ?? 'merge',
		previewSeconds: undefined,
		collisionPolicy: payload.collisionPolicy ?? 'fail',
		planSignature: 'preflight-approved',
		outputs: payload.inputFiles.map((inputPath, inputIndex) => ({
			inputIndex,
			inputPath,
			kind: 'final',
			requestedPath: `/tmp/out/${inputIndex}.m4b`,
			resolvedPath: `/tmp/out/${inputIndex}.m4b`,
			renameCandidate: undefined,
			collision: undefined,
			action: 'write',
			review: undefined,
		})),
	};
}

function successResult(jobType: ProcessCommandResult['jobType'] = 'merge'): ProcessCommandResult {
	return {
		jobType,
		summary: { total: 1, succeeded: 1, skipped: 0, cancelled: 0, failed: 0 },
		results: [
			{
				inputIndex: 0,
				status: 'success',
				message: 'ok',
				jobId: 'job-1',
				error: undefined,
				previewFilePath: undefined,
				previewActualSeconds: undefined,
			},
		],
	};
}

function workflowContext(): ProcessingWorkflowContext {
	return {
		updateStatus: vi.fn(),
		setProcessingState: vi.fn(),
		updateArtThumbnail: vi.fn(async () => undefined),
		startProgressListener: vi.fn(async () => undefined),
		setCurrentWorkKind: vi.fn(),
		setBatchCompletionMessage: vi.fn(),
		reconcileProcessResult: vi.fn(),
		handleCancellation: vi.fn(),
		resetToIdle: vi.fn(),
	};
}

function workflowServices(overrides: Partial<ProcessingWorkflowServices> = {}) {
	const feedback = { showError: vi.fn() };
	const getJobTypeMock = vi.fn((): JobType => 'merge');
	const runOutputPlanReviewWorkflowMock: ProcessingWorkflowServices['runOutputPlanReviewWorkflow'] =
		vi.fn(
			async ({ payload }): Promise<OutputPlanReviewResult> => ({
				status: 'approved',
				payload: { ...payload, preflightSignature: 'preflight-approved' },
				plan: preflightPlan(payload),
			}),
		);
	const services: ProcessingWorkflowServices = {
		updateOutputPath: vi.fn(),
		getCurrentFileList: vi.fn(() => fileList()),
		getSelectedFileIndex: vi.fn(() => 0),
		getSelectedFileIndices: vi.fn(() => new Set([0])),
		readProcessingRequestConfig: vi.fn(() => processingConfig()),
		syncEncoderPanelBeforeProcess: vi.fn(),
		getJobType: getJobTypeMock,
		hasDirtyMetadataFields: vi.fn(() => false),
		readMetadataForm: vi.fn(() => ({})),
		getAllMetadataIntentPatches: vi.fn(() => ({})),
		getMetadataForFile: vi.fn(() => undefined),
		getMetadataIntentPatchForFile: vi.fn(() => undefined),
		setMetadataForFile: vi.fn(),
		stageMetadataToSelection: vi.fn(async () => true),
		getSeriesPartValidationError: vi.fn(() => null),
		getSubseriesPartValidationError: vi.fn(() => null),
		setJobControlsEnabled: vi.fn(),
		setFileOrderLocked: vi.fn(),
		readAudioMetadata: vi.fn(async () => ({})),
		processAudiobookFiles: vi.fn(async () => successResult()),
		runOutputPlanReviewWorkflow: runOutputPlanReviewWorkflowMock,
		openGeneratedPreviewIfSingle: vi.fn(async () => undefined),
		feedback,
		console: {
			error: vi.fn(),
			log: vi.fn(),
			warn: vi.fn(),
		},
		...overrides,
	};
	return { services, feedback };
}

async function runWithServices(
	context: ProcessingWorkflowContext,
	services: ProcessingWorkflowServices,
): Promise<void> {
	await startProcessing(context, undefined, makeProcessingWorkflowServicesLayer(services));
}

describe('ProcessingWorkflow', () => {
	beforeEach(() => {
		vi.clearAllMocks();
	});

	it('coordinates approved processing through injected services without changing the public runtime API', async () => {
		const ctx = workflowContext();
		const { services } = workflowServices();

		await runWithServices(ctx, services);

		expect(services.runOutputPlanReviewWorkflow).toHaveBeenCalledTimes(1);
		expect(services.syncEncoderPanelBeforeProcess).toHaveBeenCalledTimes(1);
		expect(ctx.setProcessingState).toHaveBeenCalledWith(true);
		expect(ctx.updateStatus).toHaveBeenCalledWith({
			stage: 'analyzing',
			percentage: 0,
			message: 'Starting processing...',
		});
		expect(services.setJobControlsEnabled).toHaveBeenCalledWith(false);
		expect(services.setFileOrderLocked).toHaveBeenCalledWith(true);
		expect(ctx.updateArtThumbnail).toHaveBeenCalledTimes(1);
		expect(ctx.startProgressListener).toHaveBeenCalledTimes(1);
		expect(services.processAudiobookFiles).toHaveBeenCalledWith({
			payload: expect.objectContaining({
				inputFiles: ['/books/a.m4b'],
				preflightSignature: 'preflight-approved',
			}),
			metadataIntent: null,
			previewSeconds: undefined,
		});
		expect(ctx.reconcileProcessResult).toHaveBeenCalledWith(successResult());
		expect(ctx.setBatchCompletionMessage).toHaveBeenLastCalledWith(null);
		expect(services.updateOutputPath).toHaveBeenLastCalledWith('final');
	});

	it('stops before listener startup when output-plan review blocks processing', async () => {
		const ctx = workflowContext();
		const { services, feedback } = workflowServices({
			runOutputPlanReviewWorkflow: vi.fn(
				async ({ payload }): Promise<OutputPlanReviewResult> => ({
					status: 'blocked',
					message: 'Output path collides with source.',
					plan: preflightPlan(payload),
				}),
			),
		});

		await runWithServices(ctx, services);

		expect(feedback.showError).toHaveBeenCalledWith('Output path collides with source.');
		expect(ctx.setProcessingState).not.toHaveBeenCalled();
		expect(ctx.startProgressListener).not.toHaveBeenCalled();
		expect(services.processAudiobookFiles).not.toHaveBeenCalled();
		expect(services.updateOutputPath).toHaveBeenLastCalledWith('final');
	});

	it('routes structured cancellation failures to cancellation handling instead of error reset', async () => {
		const ctx = workflowContext();
		const { services, feedback } = workflowServices({
			processAudiobookFiles: vi.fn(async () => {
				throw {
					code: 'cancelled',
					category: 'cancellation',
					message: 'Processing was cancelled.',
					detail: 'user requested stop',
				};
			}),
		});

		await runWithServices(ctx, services);

		expect(ctx.handleCancellation).toHaveBeenCalledTimes(1);
		expect(ctx.resetToIdle).not.toHaveBeenCalled();
		expect(feedback.showError).not.toHaveBeenCalled();
		expect(services.updateOutputPath).toHaveBeenLastCalledWith('final');
	});

	it('surfaces failed processing commands through typed workflow failure handling', async () => {
		const ctx = workflowContext();
		const { services, feedback } = workflowServices({
			processAudiobookFiles: vi.fn(async () => {
				throw {
					code: 'decoder_unavailable',
					category: 'toolchain',
					message: 'Decoder unavailable.',
					detail: null,
				};
			}),
		});

		await runWithServices(ctx, services);

		expect(ctx.handleCancellation).not.toHaveBeenCalled();
		expect(feedback.showError).toHaveBeenCalledWith('Processing failed: Decoder unavailable.');
		expect(ctx.resetToIdle).toHaveBeenCalledTimes(1);
		expect(services.updateOutputPath).toHaveBeenLastCalledWith('final');
	});
});
