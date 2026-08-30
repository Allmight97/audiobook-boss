import { beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriClient } from '../../../lib/tauri/client';
import {
	makeProcessingWorkflowServicesLayer,
	startProcessing,
	type ProcessingWorkflowContext,
	type ProcessingWorkflowServices,
} from '../workflow';
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
import type { AcquisitionJob } from '../../../types/remoteSource';
import type { WorkSubmissionAccepted } from '../../../types/workRuntime';
import type { OutputPlanReviewResult } from '../../outputPlan';
import {
	purgeRemoteSourceSessionsForInputIds,
	registerRemoteSourceSupplementalAssets,
	supplementalAssetsByInputIdForProcessing,
} from '../../../ui/remoteSource';
import { resetRemoteSourceSessionAssets } from '../../remoteSource';

function audioFile(path: string, overrides: Partial<AudioFile> = {}): AudioFile {
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
		...overrides,
	};
}

function fileList(paths = ['/books/a.m4b']): FileListInfo {
	return {
		files: paths.map((path) => audioFile(path)),
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
		terminalClass: 'success',
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

function acceptedSubmission(jobType: JobType = 'merge'): WorkSubmissionAccepted {
	return {
		operationId: 'operation-1',
		snapshot: {
			operationId: 'operation-1',
			sequence: 1,
			kind: jobType === 'batch' ? 'processingBatch' : 'processingMerge',
			status: 'accepted',
			title: jobType === 'batch' ? 'Batch encode (1 file)' : 'Merge encode (1 file)',
			createdAtMs: 1,
			startedAtMs: undefined,
			finishedAtMs: undefined,
			cancellable: true,
			cancelRequested: false,
			lanes: ['analysis', 'encodeCpu', 'outputCommit'],
			sourceInputIds: ['current-input-1'],
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
			logTail: [],
		},
	};
}

function acquisitionJobWithPdf(): AcquisitionJob {
	return {
		jobId: 'remote-job-1',
		providerId: 'audible',
		status: 'validated',
		progress: {
			stage: 'importHandoff',
			percentage: 100,
			message: 'Ready for import.',
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			currentTitleId: 'B000000001',
			currentItemIndex: 1,
			totalItems: 1,
			terminal: true,
		},
		materializedFiles: [
			{
				inputId: 'provider-input-1',
				titleId: 'B000000001',
				path: '/session/book.m4b',
				sizeBytes: 1024,
				sha256: 'audio-sha',
			},
		],
		supplementalAssets: [
			{
				assetId: 'pdf-1',
				inputId: 'provider-input-1',
				titleId: 'B000000001',
				path: '/session/book.pdf',
				fileName: 'Being You - A New Science of Consciousness - Supplemental PDF.pdf',
				sizeBytes: 32,
				sha256: 'pdf-sha',
			},
		],
		diagnostics: [],
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
		getCurrentFileList: vi.fn(() => fileList()),
		getSelectedFileIndex: vi.fn(() => 0),
		getSelectedFileIndices: vi.fn(() => new Set([0])),
		readProcessingRequestConfig: vi.fn(() => processingConfig()),
		getJobType: getJobTypeMock,
		hasDirtyMetadataFields: vi.fn(() => false),
		readMetadataForm: vi.fn(() => ({})),
		collectActionableMetadataIntent: vi.fn(() => null),
		getMetadataForFile: vi.fn(() => undefined),
		cacheMetadataForFile: vi.fn(),
		stageMetadataIntentPatch: vi.fn(() => 'staged' as const),
		stageMetadataToSelection: vi.fn(async () => true),
		setJobControlsEnabled: vi.fn(),
		setFileOrderLocked: vi.fn(),
		validateMetadataIntentPatch: vi.fn(async (metadataPatch) => ({
			isValid: true,
			metadataPatch,
			fieldErrors: [],
		})),
		readAudioMetadata: vi.fn(async () => ({})),
		processAudiobookFiles: vi.fn(async () => successResult()),
		submitProcessingOperation: vi.fn(async () => acceptedSubmission(getJobTypeMock())),
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
		resetRemoteSourceSessionAssets();
	});

	it('coordinates approved processing through injected services without changing the public runtime API', async () => {
		const ctx = workflowContext();
		const { services } = workflowServices();

		await runWithServices(ctx, services);

		expect(services.runOutputPlanReviewWorkflow).toHaveBeenCalledTimes(1);
		expect(ctx.setProcessingState).toHaveBeenCalledWith(true);
		expect(ctx.updateStatus).toHaveBeenCalledWith({
			stage: 'analyzing',
			percentage: 0,
			message: 'Starting processing...',
		});
		expect(services.setJobControlsEnabled).toHaveBeenCalledWith(false);
		expect(services.setFileOrderLocked).toHaveBeenCalledWith(true);
		expect(ctx.updateArtThumbnail).toHaveBeenCalledTimes(1);
		expect(ctx.startProgressListener).not.toHaveBeenCalled();
		expect(services.submitProcessingOperation).toHaveBeenCalledWith({
			payload: expect.objectContaining({
				inputFiles: ['/books/a.m4b'],
				preflightSignature: 'preflight-approved',
			}),
			metadataIntent: null,
			previewSeconds: undefined,
		});
		expect(services.processAudiobookFiles).not.toHaveBeenCalled();
		expect(ctx.reconcileProcessResult).not.toHaveBeenCalled();
		expect(ctx.setProcessingState).toHaveBeenLastCalledWith(false);
		expect(services.setJobControlsEnabled).toHaveBeenLastCalledWith(true);
		expect(services.setFileOrderLocked).toHaveBeenLastCalledWith(false);
		expect(ctx.setBatchCompletionMessage).toHaveBeenLastCalledWith(null);
	});

	it('submits files in the current file-list order', async () => {
		const ctx = workflowContext();
		const { services } = workflowServices({
			getCurrentFileList: vi.fn(() => ({
				files: [
					audioFile('/books/2 - Early Chapter.mp3', { inputId: 'second' }),
					audioFile('/books/10 - Last Chapter.mp3', { inputId: 'tenth' }),
				],
				selectedDecoders: [null, null],
				totalDuration: 2,
				totalSize: 2,
				validCount: 2,
				invalidCount: 0,
			})),
		});
		await runWithServices(ctx, services);

		expect(services.submitProcessingOperation).toHaveBeenCalledWith(
			expect.objectContaining({
				payload: expect.objectContaining({
					inputFiles: ['/books/2 - Early Chapter.mp3', '/books/10 - Last Chapter.mp3'],
					inputIds: ['second', 'tenth'],
				}),
			}),
		);
	});

	it('passes acquired supplemental PDF assets into processing payload by FileList input id', async () => {
		const currentFileList: FileListInfo = {
			files: [audioFile('/session/book.m4b', { inputId: 'current-input-1' })],
			selectedDecoders: [null],
			totalDuration: 1,
			totalSize: 1,
			validCount: 1,
			invalidCount: 0,
		};
		registerRemoteSourceSupplementalAssets(acquisitionJobWithPdf(), currentFileList);
		const ctx = workflowContext();
		const { services } = workflowServices({
			getCurrentFileList: vi.fn(() => currentFileList),
			getJobType: vi.fn((): JobType => 'batch'),
		});

		await runWithServices(ctx, services);

		expect(services.submitProcessingOperation).toHaveBeenCalledWith({
			payload: expect.objectContaining({
				inputFiles: ['/session/book.m4b'],
				inputIds: ['current-input-1'],
				jobType: 'batch',
				supplementalAssetsByInputId: {
					'current-input-1': [
						{
							assetId: 'pdf-1',
							inputId: 'current-input-1',
							titleId: 'B000000001',
							path: '/session/book.pdf',
							fileName: 'Being You - A New Science of Consciousness - Supplemental PDF.pdf',
							sizeBytes: 32,
							sha256: 'pdf-sha',
						},
					],
				},
			}),
			metadataIntent: null,
			previewSeconds: undefined,
		});
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
		expect(services.submitProcessingOperation).not.toHaveBeenCalled();
	});

	it('routes structured cancellation failures to cancellation handling instead of error reset', async () => {
		const ctx = workflowContext();
		const { services, feedback } = workflowServices({
			submitProcessingOperation: vi.fn(async () => {
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
	});

	it('surfaces failed processing commands through typed workflow failure handling', async () => {
		const ctx = workflowContext();
		const { services, feedback } = workflowServices({
			submitProcessingOperation: vi.fn(async () => {
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
	});

	it('keeps retained remote-source sessions when submission fails without a pending purge', async () => {
		const currentFileList: FileListInfo = {
			files: [audioFile('/session/book.m4b', { inputId: 'current-input-1' })],
			selectedDecoders: [null],
			totalDuration: 1,
			totalSize: 1,
			validCount: 1,
			invalidCount: 0,
		};
		registerRemoteSourceSupplementalAssets(acquisitionJobWithPdf(), currentFileList);
		const purgeSpy = vi.spyOn(tauriClient, 'purgeRemoteSourceSession').mockResolvedValue(undefined);
		const ctx = workflowContext();
		const { services, feedback } = workflowServices({
			getCurrentFileList: vi.fn(() => currentFileList),
			getJobType: vi.fn((): JobType => 'batch'),
			submitProcessingOperation: vi.fn(async () => {
				throw {
					code: 'decoder_unavailable',
					category: 'toolchain',
					message: 'Decoder unavailable.',
					detail: null,
				};
			}),
		});

		await runWithServices(ctx, services);

		expect(feedback.showError).toHaveBeenCalledWith('Processing failed: Decoder unavailable.');
		expect(purgeSpy).not.toHaveBeenCalled();
		expect(supplementalAssetsByInputIdForProcessing(['current-input-1'])).toBeDefined();
	});

	it('purges retained remote-source sessions that were pending purge when submission fails', async () => {
		const currentFileList: FileListInfo = {
			files: [audioFile('/session/book.m4b', { inputId: 'current-input-1' })],
			selectedDecoders: [null],
			totalDuration: 1,
			totalSize: 1,
			validCount: 1,
			invalidCount: 0,
		};
		registerRemoteSourceSupplementalAssets(acquisitionJobWithPdf(), currentFileList);
		const purgeSpy = vi.spyOn(tauriClient, 'purgeRemoteSourceSession').mockResolvedValue(undefined);
		const ctx = workflowContext();
		const { services, feedback } = workflowServices({
			getCurrentFileList: vi.fn(() => currentFileList),
			getJobType: vi.fn((): JobType => 'batch'),
			submitProcessingOperation: vi.fn(async () => {
				await purgeRemoteSourceSessionsForInputIds(['current-input-1']);
				throw {
					code: 'decoder_unavailable',
					category: 'toolchain',
					message: 'Decoder unavailable.',
					detail: null,
				};
			}),
		});

		await runWithServices(ctx, services);

		expect(feedback.showError).toHaveBeenCalledWith('Processing failed: Decoder unavailable.');
		expect(purgeSpy).toHaveBeenCalledWith('remote-job-1');
		expect(supplementalAssetsByInputIdForProcessing(['current-input-1'])).toBeUndefined();
	});
});
