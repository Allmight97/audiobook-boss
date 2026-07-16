import { beforeEach, describe, expect, it, vi } from 'vitest';

import {
	defaultEncoderSettings,
	type AudioFile,
	type FileListInfo,
	type JobType,
	type ProcessPayload,
	type ProcessingPreflightPlan,
	type ProcessingRequestConfig,
} from '../../types/audio';
import type { OnlineMetadataResult } from '../../types/metadata';
import type { WorkSubmissionAccepted } from '../../types/workRuntime';
import { applyMetadataToForm, populateMetadataFormSingle, readMetadataForm } from '../metadataForm';
import {
	cacheMetadataForFile,
	clearMetadataSession,
	collectActionableMetadataIntent,
	getMetadataForFile,
	stageMetadataIntentPatch,
} from '../metadataSession';
import {
	makeMetadataLookupWorkflowServicesLayer,
	runMetadataLookupWorkflow,
	type MetadataLookupWorkflowServices,
} from '../metadataLookup/metadataLookupWorkflow';
import type { MetadataLookupQueueState, MetadataLookupState } from '../metadataLookup/state.svelte';
import {
	makeProcessingWorkflowServicesLayer,
	startProcessing,
	type ProcessingWorkflowContext,
	type ProcessingWorkflowServices,
} from '../statusPanel/processingWorkflow';
import type { OutputPlanReviewResult } from '../outputPanel';

vi.mock('../coverArt', () => ({
	clearCoverArt: vi.fn(),
	getCurrentCoverArt: vi.fn(() => null),
	getHasCustomCoverArt: vi.fn(() => false),
	isCoverArtRemovalRequested: vi.fn(() => false),
	refreshCoverArtDisplay: vi.fn(),
	setCoverArt: vi.fn(),
	setCustomCoverArt: vi.fn(),
}));

vi.mock('../tagPreview', () => ({
	calculateTSOA: vi.fn(() => ''),
	initTagPreview: vi.fn(),
	updateTagPreview: vi.fn(),
}));

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

function fileList(files: AudioFile[]): FileListInfo {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length,
		totalSize: files.length,
		validCount: files.filter((file) => file.isValid).length,
		invalidCount: files.filter((file) => !file.isValid).length,
	};
}

function lookupResult(): OnlineMetadataResult {
	return {
		source: 'audnexus',
		sourceId: 'audnexus:workflow-smoke',
		title: 'Looked Up Title',
		authors: ['Author Person'],
		narrators: ['Narrator Person'],
		description: 'A searched description.',
		publishedDate: '2024-07',
		durationSeconds: 123,
		coverUrl: 'https://example.com/cover.jpg',
		audibleOnly: false,
	};
}

function processingConfig(): ProcessingRequestConfig {
	const encoderSettings = {
		...defaultEncoderSettings(),
		bitrateKbps: 96,
	};
	return {
		encoderSettings,
		sampleRate: 'auto',
		outputDirectory: '/tmp/out',
		outputNaming: { preset: 'absDefault', includeYear: true, customTemplate: undefined },
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

function acceptedSubmission(jobType: JobType = 'merge'): WorkSubmissionAccepted {
	return {
		operationId: 'operation-smoke',
		snapshot: {
			operationId: 'operation-smoke',
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
			sourceInputIds: [],
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

describe('UI workflow smoke', () => {
	const file = audioFile('/books/alpha.m4b');
	const currentFileList = fileList([file]);

	beforeEach(() => {
		clearMetadataSession();
		populateMetadataFormSingle({});
		cacheMetadataForFile(file.path, { title: 'Existing Title' });
	});

	it('composes lookup metadata, output review, and final processing into the IPC payload', async () => {
		const lookupState: MetadataLookupState = {
			isOpen: true,
			titleQuery: 'looked up',
			authorQuery: '',
			source: 'auto',
			applyMode: 'queue',
			replaceCoverArt: true,
			statusMessage: '',
			statusVariant: 'info',
			queueContext: '',
			results: [lookupResult()],
			isQueueMode: true,
			skipEnabled: true,
			hasSearched: true,
		};
		const queueState: MetadataLookupQueueState = {
			queue: [{ file, index: 0 }],
			index: 0,
		};
		const lookupServices: MetadataLookupWorkflowServices = {
			getLookupState: () => lookupState,
			getQueueState: () => queueState,
			setMetadataLookupQueue: vi.fn(),
			clearMetadataLookupQueue: vi.fn(),
			setMetadataLookupQueueIndex: vi.fn((index) => {
				queueState.index = index;
			}),
			getSelectedFileIndices: () => new Set([0]),
			getCurrentFileList: () => currentFileList,
			getMetadataForFile,
			stageMetadataIntentPatch,
			selectFile: vi.fn(async () => undefined),
			applyMetadataToForm,
			readMetadataForm,
			updateOutputPath: vi.fn(),
			updateEstimatedSize: vi.fn(),
			updateTagPreview: vi.fn(),
			clearCoverArt: vi.fn(),
			setCoverArt: vi.fn(),
			setCustomCoverArt: vi.fn(),
			refreshCoverArtDisplay: vi.fn(),
			searchOnlineMetadata: vi.fn(),
			loadCoverArtFromUrl: vi.fn(async () => [9, 9, 9]),
			focusElementById: vi.fn(),
			queueMicrotask: (callback) => callback(),
			console: {
				error: vi.fn(),
				warn: vi.fn(),
			},
		};

		await runMetadataLookupWorkflow(makeMetadataLookupWorkflowServicesLayer(lookupServices), {
			type: 'applyResult',
			index: 0,
		});

		expect(collectActionableMetadataIntent([file.path])?.[file.path]).toMatchObject({
			title: { op: 'set', value: 'Looked Up Title' },
			artist: { op: 'set', value: 'Author Person' },
			composer: { op: 'set', value: 'Narrator Person' },
			cover_art: { op: 'set', value: [9, 9, 9] },
		});

		const processingRequestConfig = processingConfig();
		const submitProcessingOperation = vi.fn(async () => acceptedSubmission('merge'));
		const processAudiobookFiles = vi.fn();
		const runOutputPlanReviewWorkflow: ProcessingWorkflowServices['runOutputPlanReviewWorkflow'] =
			vi.fn(
				async ({ payload }): Promise<OutputPlanReviewResult> => ({
					status: 'approved',
					payload: { ...payload, preflightSignature: 'preflight-approved' },
					plan: preflightPlan(payload),
				}),
			);
		const processingServices: ProcessingWorkflowServices = {
			updateOutputPath: vi.fn(),
			getCurrentFileList: vi.fn(() => currentFileList),
			getSelectedFileIndex: vi.fn(() => 0),
			getSelectedFileIndices: vi.fn(() => new Set([0])),
			readProcessingRequestConfig: vi.fn(() => processingRequestConfig),
			getJobType: vi.fn((): JobType => 'merge'),
			hasDirtyMetadataFields: vi.fn(() => true),
			readMetadataForm,
			collectActionableMetadataIntent,
			getMetadataForFile,
			cacheMetadataForFile,
			stageMetadataIntentPatch,
			stageMetadataToSelection: vi.fn(async () => true),
			setJobControlsEnabled: vi.fn(),
			setFileOrderLocked: vi.fn(),
			validateMetadataIntentPatch: vi.fn(async (metadataPatch) => ({
				isValid: true,
				metadataPatch,
				fieldErrors: [],
			})),
			readAudioMetadata: vi.fn(async () => ({})),
			processAudiobookFiles,
			submitProcessingOperation,
			runOutputPlanReviewWorkflow,
			openGeneratedPreviewIfSingle: vi.fn(async () => undefined),
			feedback: { showError: vi.fn() },
			console: {
				error: vi.fn(),
				log: vi.fn(),
				warn: vi.fn(),
			},
		};

		await startProcessing(
			workflowContext(),
			undefined,
			makeProcessingWorkflowServicesLayer(processingServices),
		);

		expect(runOutputPlanReviewWorkflow).toHaveBeenCalledWith({
			payload: expect.objectContaining({
				inputFiles: [file.path],
				inputIds: [null],
				outputDir: '/tmp/out',
				settings: processingRequestConfig.encoderSettings,
				sampleRate: 'auto',
				jobType: 'merge',
				outputNaming: processingRequestConfig.outputNaming,
			}),
			metadataIntentByPath: expect.objectContaining({
				[file.path]: expect.objectContaining({
					cover_art: { op: 'set', value: [9, 9, 9] },
				}),
			}),
			previewSeconds: undefined,
		});
		expect(submitProcessingOperation).toHaveBeenCalledTimes(1);
		expect(submitProcessingOperation).toHaveBeenCalledWith({
			payload: expect.objectContaining({
				inputFiles: [file.path],
				inputIds: [null],
				outputDir: '/tmp/out',
				settings: processingRequestConfig.encoderSettings,
				sampleRate: 'auto',
				jobType: 'merge',
				outputNaming: processingRequestConfig.outputNaming,
				preflightSignature: 'preflight-approved',
			}),
			metadataIntent: {
				[file.path]: expect.objectContaining({
					title: { op: 'set', value: 'Looked Up Title' },
					album: { op: 'set', value: 'Looked Up Title' },
					artist: { op: 'set', value: 'Author Person' },
					composer: { op: 'set', value: 'Narrator Person' },
					description: { op: 'set', value: 'A searched description.' },
					date: { op: 'set', value: '2024-07' },
					cover_art: { op: 'set', value: [9, 9, 9] },
				}),
			},
			previewSeconds: undefined,
		});
		expect(processAudiobookFiles).not.toHaveBeenCalled();
	});
});
