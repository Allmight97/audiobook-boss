import { Effect, type AppEffect } from '../../lib/effect/appEffect';
import type {
	FileListInfo,
	JobType,
	ProcessPayload,
	ProcessingRequestConfig,
} from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { isUsableMetadataCache, validateMetadataDraft } from '../metadataSession';
import type { OutputPlanReviewResult } from '../outputPanel';
import type { ProcessingWorkflowFailed } from './processingWorkflow';
import type { ProcessingWorkflowServices } from './processingWorkflow';
import { supplementalAssetsByInputIdForProcessing } from '../remoteSource';

type MetadataIntentByPath = Record<string, MetadataIntentPatch>;

type ProcessingWorkflowPromise = <A>(
	evaluate: () => PromiseLike<A>,
	message: string,
) => AppEffect<A, ProcessingWorkflowFailed>;

export function validInputFilePaths(fileList: FileListInfo): string[] {
	return fileList.files.filter((file) => file.isValid).map((file) => file.path);
}

export function validInputIds(fileList: FileListInfo): (string | undefined)[] {
	return fileList.files.filter((file) => file.isValid).map((file) => file.inputId);
}

function toWireInputIds(inputIds: readonly (string | undefined)[]): (string | null)[] {
	return inputIds.map((inputId) => inputId ?? null);
}

export function buildProcessPayload(
	filePaths: string[],
	inputIds: (string | undefined)[],
	processingRequestConfig: ProcessingRequestConfig,
	jobType: JobType,
): ProcessPayload {
	return {
		inputFiles: filePaths,
		inputIds: toWireInputIds(inputIds),
		outputDir: processingRequestConfig.outputDirectory,
		settings: processingRequestConfig.encoderSettings,
		sampleRate: processingRequestConfig.sampleRate,
		jobType,
		outputNaming: processingRequestConfig.outputNaming,
		supplementalAssetsByInputId: supplementalAssetsByInputIdForProcessing(inputIds),
	};
}

function stageMultiSelectionMetadata(
	services: ProcessingWorkflowServices,
	selectionCount: number,
	workflowPromise: ProcessingWorkflowPromise,
): AppEffect<boolean, ProcessingWorkflowFailed> {
	return Effect.gen(function* () {
		if (selectionCount <= 1) {
			return true;
		}

		const staged = yield* workflowPromise(
			() => services.stageMetadataToSelection({ showStatus: false }),
			'Failed to stage metadata for processing.',
		);
		if (!staged) {
			yield* Effect.sync(() =>
				services.feedback.showError('Fix metadata validation errors before processing.'),
			);
			return false;
		}
		return true;
	});
}

function metadataIntentTargetPath(
	services: ProcessingWorkflowServices,
	fileList: FileListInfo,
	selectedFileIndex: number,
): string | undefined {
	if (services.getJobType() === 'merge') {
		return validInputFilePaths(fileList)[0];
	}

	if (selectedFileIndex >= 0) {
		const selectedFile = fileList.files[selectedFileIndex];
		if (selectedFile?.isValid) {
			return selectedFile.path;
		}
	}

	return undefined;
}

function stageSingleSelectionMetadata(
	services: ProcessingWorkflowServices,
	fileList: FileListInfo,
	selectionCount: number,
	workflowPromise: ProcessingWorkflowPromise,
): AppEffect<boolean, ProcessingWorkflowFailed> {
	return Effect.gen(function* () {
		if (selectionCount > 1 || !services.hasDirtyMetadataFields()) {
			return true;
		}

		const selectedFileIndex = services.getSelectedFileIndex();
		const formMetadata = services.readMetadataForm({ mode: 'single' });
		const validation = yield* workflowPromise(
			() => validateMetadataDraft(formMetadata, services.validateMetadataIntentPatch),
			'Failed to validate metadata intent for processing.',
		);
		if (!validation.ok) {
			const validationError = validation.errors.first ?? 'Metadata validation failed.';
			yield* Effect.sync(() => services.feedback.showError(validationError));
			return false;
		}

		const intentPatch = validation.intentPatch;
		const targetPath = metadataIntentTargetPath(services, fileList, selectedFileIndex);
		if (!targetPath) {
			yield* Effect.sync(() =>
				services.feedback.showError('Select a valid input file before processing metadata edits.'),
			);
			return false;
		}
		services.stageMetadataIntentPatch(targetPath, intentPatch);

		return true;
	});
}

export function stagePendingMetadataIntent(
	services: ProcessingWorkflowServices,
	fileList: FileListInfo,
	workflowPromise: ProcessingWorkflowPromise,
): AppEffect<boolean, ProcessingWorkflowFailed> {
	return Effect.gen(function* () {
		const selectionCount = services.getSelectedFileIndices().size;
		const multiSelectionStaged = yield* stageMultiSelectionMetadata(
			services,
			selectionCount,
			workflowPromise,
		);
		if (!multiSelectionStaged) {
			return false;
		}

		return yield* stageSingleSelectionMetadata(services, fileList, selectionCount, workflowPromise);
	});
}

export function ensureBatchMetadataLoaded(
	services: ProcessingWorkflowServices,
	processPayload: ProcessPayload,
	workflowPromise: ProcessingWorkflowPromise,
): AppEffect<void, ProcessingWorkflowFailed> {
	return Effect.gen(function* () {
		if (processPayload.jobType !== 'batch') {
			return;
		}

		const missingMetadata = processPayload.inputFiles.filter(
			(filePath) => !isUsableMetadataCache(services.getMetadataForFile(filePath)),
		);
		if (missingMetadata.length === 0) {
			return;
		}

		yield* workflowPromise(
			() =>
				Promise.all(
					missingMetadata.map(async (filePath) => {
						try {
							const metadata = await services.readAudioMetadata(filePath);
							services.cacheMetadataForFile(filePath, metadata);
						} catch (error) {
							services.console.warn('Failed to load metadata for batch file:', filePath, error);
						}
					}),
				),
			'Failed to load batch metadata.',
		);
	});
}

export function buildMetadataIntentByPath(
	services: ProcessingWorkflowServices,
	processPayload: ProcessPayload,
): MetadataIntentByPath | null {
	if (processPayload.jobType === 'merge') {
		const mergeKey = processPayload.inputFiles[0];
		return mergeKey ? services.collectActionableMetadataIntent([mergeKey]) : null;
	}

	return services.collectActionableMetadataIntent(processPayload.inputFiles);
}

export function reviewOutputPlan(
	services: ProcessingWorkflowServices,
	request: {
		payload: ProcessPayload;
		metadataIntentByPath: MetadataIntentByPath | null;
		previewSeconds?: number;
	},
	workflowPromise: ProcessingWorkflowPromise,
): AppEffect<OutputPlanReviewResult, ProcessingWorkflowFailed> {
	return workflowPromise(
		() =>
			services.runOutputPlanReviewWorkflow({
				payload: request.payload,
				metadataIntentByPath: request.metadataIntentByPath,
				previewSeconds: request.previewSeconds,
			}),
		'Output plan review failed.',
	);
}
