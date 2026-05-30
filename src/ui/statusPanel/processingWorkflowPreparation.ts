import { Effect, type AppEffect } from '../../lib/effect/appEffect';
import type {
	FileListInfo,
	JobType,
	ProcessPayload,
	ProcessingRequestConfig,
} from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { applyMetadataDraftIntent, hasActionableMetadataDraftIntent } from '../metadataDraft';
import {
	firstMetadataIntentValidationError,
	validateMetadataDraftIntent,
} from '../metadataValidation';
import type { OutputPlanReviewResult } from '../outputPanel/outputPlanWorkflow';
import type { ProcessingWorkflowFailed } from './processingWorkflow';
import type { ProcessingWorkflowServices } from './processingWorkflowServices';

type MetadataIntentByPath = Record<string, MetadataIntentPatch>;

type ProcessingWorkflowPromise = <A>(
	evaluate: () => PromiseLike<A>,
	message: string,
) => AppEffect<A, ProcessingWorkflowFailed>;

export function validInputFilePaths(fileList: FileListInfo): string[] {
	return fileList.files.filter((file) => file.isValid).map((file) => file.path);
}

export function buildProcessPayload(
	filePaths: string[],
	processingRequestConfig: ProcessingRequestConfig,
	jobType: JobType,
): ProcessPayload {
	return {
		inputFiles: filePaths,
		outputDir: processingRequestConfig.outputDirectory,
		settings: processingRequestConfig.encoderSettings,
		externalToolchain: processingRequestConfig.toolchainSettings,
		sampleRate: processingRequestConfig.sampleRate,
		jobType,
		outputNaming: processingRequestConfig.outputNaming,
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
			() => validateMetadataDraftIntent(formMetadata, services.validateMetadataIntentPatch),
			'Failed to validate metadata intent for processing.',
		);
		const validationError = firstMetadataIntentValidationError(validation.result);
		if (validationError) {
			yield* Effect.sync(() => services.feedback.showError(validationError));
			return false;
		}

		const intentPatch = validation.intentPatch;
		const activeFile =
			selectedFileIndex >= 0
				? fileList.files[selectedFileIndex]
				: fileList.files.find((file) => file.isValid);
		if (activeFile?.isValid && hasActionableMetadataDraftIntent(intentPatch)) {
			const existing = services.getMetadataForFile(activeFile.path) ?? {};
			const currentMetadata = applyMetadataDraftIntent(existing, intentPatch);
			yield* Effect.sync(() =>
				services.setMetadataForFile(activeFile.path, currentMetadata, {
					markPending: true,
					intentPatch,
				}),
			);
		}

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
			(filePath) => !services.getMetadataForFile(filePath),
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
							services.setMetadataForFile(filePath, metadata);
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
		const mergeIntentPatch = mergeKey
			? services.getMetadataIntentPatchForFile(mergeKey)
			: undefined;
		if (
			mergeKey &&
			processPayload.inputFiles.length > 0 &&
			hasActionableMetadataDraftIntent(mergeIntentPatch)
		) {
			return {
				[mergeKey]: mergeIntentPatch,
			};
		}
		return null;
	}

	const storedMetadataIntentByPath = services.getAllMetadataIntentPatches();
	const activeInputFiles = new Set(processPayload.inputFiles);
	const filteredMetadataIntentByPath = Object.fromEntries(
		Object.entries(storedMetadataIntentByPath).filter(
			([filePath, value]) =>
				activeInputFiles.has(filePath) && hasActionableMetadataDraftIntent(value),
		),
	);
	return Object.keys(filteredMetadataIntentByPath).length > 0 ? filteredMetadataIntentByPath : null;
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
