import type {
	ProcessCommandResult,
	ProcessPayload,
	ProcessingRequestConfig,
} from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { isAppErrorCategory, normalizeAppError } from '../../lib/tauri/appError';
import {
	Data,
	Effect,
	type AppEffect,
	runAppEffect,
	workflowTryPromise,
} from '../../lib/effect/appEffect';
import {
	ProcessingWorkflowServicesTag,
	type ProcessingWorkflowLayer,
	type ProcessingWorkflowServicesId,
	type ProcessingWorkflowServices,
} from './processingWorkflowServices';
import {
	buildMetadataIntentByPath,
	buildProcessPayload,
	ensureBatchMetadataLoaded,
	reviewOutputPlan,
	stagePendingMetadataIntent,
	validInputFilePaths,
} from './processingWorkflowPreparation';
import type { ProcessingStatus } from './state';

type MetadataIntentByPath = Record<string, MetadataIntentPatch>;

export {
	ProcessingWorkflowServicesTag,
	makeProcessingWorkflowServicesLayer,
	type ProcessingWorkflowLayer,
	type ProcessingWorkflowServicesId,
	type ProcessingWorkflowServices,
} from './processingWorkflowServices';

export interface ProcessingWorkflowContext {
	updateStatus: (status: ProcessingStatus) => void;
	setProcessingState: (isProcessing: boolean) => void;
	updateArtThumbnail: () => Promise<void>;
	startProgressListener: () => Promise<void>;
	setCurrentWorkKind: (workKind: 'merge' | 'batch') => void;
	setBatchCompletionMessage: (message: string | null) => void;
	reconcileProcessResult?: (result: ProcessCommandResult) => void;
	handleCancellation: () => void;
	resetToIdle: () => void;
}

export class ProcessingWorkflowFailed extends Data.TaggedError('ProcessingWorkflowFailed')<{
	readonly message: string;
	readonly cause: unknown;
}> {}

export class ProcessingWorkflowCancelled extends Data.TaggedError('ProcessingWorkflowCancelled')<{
	readonly message: string;
	readonly cause: unknown;
}> {}

export type ProcessingWorkflowError = ProcessingWorkflowFailed | ProcessingWorkflowCancelled;

function errorDisplayText(error: unknown): string {
	if (typeof error === 'string') {
		return error;
	}
	return String(error);
}

function summarizeBatchOutcome(result: ProcessCommandResult, filePaths: string[]): string | null {
	if (result.jobType !== 'batch') {
		return null;
	}

	const total = result.summary?.total ?? result.results.length;
	const succeeded =
		result.summary?.succeeded ??
		result.results.filter((entry) => entry.status === 'success').length;
	const skipped =
		result.summary?.skipped ?? result.results.filter((entry) => entry.status === 'skipped').length;
	const cancelled =
		result.summary?.cancelled ??
		result.results.filter((entry) => entry.status === 'cancelled').length;
	const failed =
		result.summary?.failed ?? result.results.filter((entry) => entry.status === 'failed').length;

	if (failed <= 0 && skipped <= 0 && cancelled <= 0) {
		return null;
	}

	const failedNames = Array.from(
		new Set(
			result.results
				.filter((entry) => entry.status === 'failed')
				.map((entry) => {
					if (typeof entry.inputIndex === 'number') {
						const path = filePaths[entry.inputIndex];
						if (path) {
							return path.split(/[\\/]/).pop() || path;
						}
					}
					if (entry.error != null) {
						const errorMessage = normalizeAppError(entry.error, '').message;
						if (errorMessage.length > 0) {
							return errorMessage;
						}
					}
					if (typeof entry.message === 'string' && entry.message.length > 0) {
						return entry.message;
					}
					return 'Unknown failure';
				}),
		),
	);

	const visibleNames = failedNames.slice(0, 2);
	const moreCount = Math.max(0, failed - visibleNames.length);
	const failureSuffix =
		visibleNames.length > 0
			? ` Failed: ${visibleNames.join(', ')}${moreCount > 0 ? ` (+${moreCount} more)` : ''}`
			: '';
	const skippedSuffix = skipped > 0 ? ` Skipped: ${skipped}.` : '';
	const cancelledSuffix = cancelled > 0 ? ` Cancelled: ${cancelled}.` : '';

	if (succeeded <= 0) {
		return `No files were processed successfully.${skippedSuffix}${cancelledSuffix}${failureSuffix}`;
	}

	return `Processed ${succeeded}/${total}.${skippedSuffix}${cancelledSuffix}${failureSuffix}`;
}

function workflowFailure(message: string, cause: unknown): ProcessingWorkflowFailed {
	const normalized = normalizeAppError(cause, message);
	return new ProcessingWorkflowFailed({
		message: normalized.message || message,
		cause,
	});
}

function workflowPromise<A>(
	evaluate: () => PromiseLike<A>,
	message: string,
): AppEffect<A, ProcessingWorkflowFailed> {
	return workflowTryPromise(evaluate, message, workflowFailure);
}

function processingCommand(
	services: ProcessingWorkflowServices,
	request: {
		payload: ProcessPayload;
		metadataIntentByPath: MetadataIntentByPath | null;
		previewSeconds?: number;
	},
): AppEffect<ProcessCommandResult, ProcessingWorkflowError> {
	return Effect.tryPromise({
		try: () =>
			services.processAudiobookFiles({
				payload: request.payload,
				metadataIntent: request.metadataIntentByPath,
				previewSeconds: request.previewSeconds,
			}),
		catch: (cause) => {
			const normalized = normalizeAppError(cause);
			const wasCancelled =
				isAppErrorCategory(cause, 'cancellation') ||
				normalized.message.toLowerCase().includes('cancelled');
			if (wasCancelled) {
				return new ProcessingWorkflowCancelled({
					message: normalized.message,
					cause,
				});
			}
			return new ProcessingWorkflowFailed({
				message: normalized.message,
				cause,
			});
		},
	});
}

function readProcessingConfig(
	services: ProcessingWorkflowServices,
): AppEffect<ProcessingRequestConfig | null> {
	return Effect.try({
		try: () => services.readProcessingRequestConfig(),
		catch: (cause) => cause,
	}).pipe(
		Effect.catchAll((error) =>
			Effect.sync(() => {
				services.console.log('StatusPanel: Settings validation failed:', error);
				services.feedback.showError(`Settings validation failed: ${errorDisplayText(error)}`);
				return null;
			}),
		),
	);
}

function beginProcessingExecution(
	services: ProcessingWorkflowServices,
	context: ProcessingWorkflowContext,
): AppEffect<void> {
	return Effect.sync(() => {
		context.setProcessingState(true);
		context.updateStatus({
			stage: 'analyzing',
			percentage: 0,
			message: 'Starting processing...',
		});
		services.setJobControlsEnabled(false);
		services.setFileOrderLocked(true);
	});
}

function startProcessingRuntime(
	context: ProcessingWorkflowContext,
): AppEffect<void, ProcessingWorkflowFailed> {
	return Effect.gen(function* () {
		yield* workflowPromise(() => context.updateArtThumbnail(), 'Failed to update art thumbnail.');
		yield* workflowPromise(
			() => context.startProgressListener(),
			'Failed to start progress listener.',
		);
	});
}

function completeProcessingExecution(
	services: ProcessingWorkflowServices,
	context: ProcessingWorkflowContext,
	result: ProcessCommandResult,
	filePaths: string[],
): AppEffect<void, ProcessingWorkflowFailed> {
	return Effect.gen(function* () {
		yield* Effect.sync(() => {
			services.console.log('Processing command resolved:', result);
			context.reconcileProcessResult?.(result);
			context.setBatchCompletionMessage(summarizeBatchOutcome(result, filePaths));
		});
		yield* workflowPromise(
			() => services.openGeneratedPreviewIfSingle(result),
			'Failed to open generated preview.',
		);
	});
}

function handleWorkflowError(
	services: ProcessingWorkflowServices,
	context: ProcessingWorkflowContext,
	error: ProcessingWorkflowError,
): AppEffect<void> {
	return Effect.sync(() => {
		if (error._tag === 'ProcessingWorkflowCancelled') {
			context.handleCancellation();
			return;
		}
		services.console.error('Processing failed:', error.cause);
		services.feedback.showError(`Processing failed: ${error.message}`);
		context.resetToIdle();
	});
}

export function processingWorkflowProgram(
	context: ProcessingWorkflowContext,
	options?: {
		previewSeconds?: number;
	},
): AppEffect<void, never, ProcessingWorkflowServicesId> {
	const outputKind = options?.previewSeconds == null ? 'final' : 'preview';

	return Effect.gen(function* () {
		const services = yield* ProcessingWorkflowServicesTag;

		yield* Effect.sync(() => {
			services.updateOutputPath(outputKind);
			services.console.log('StatusPanel: Starting processing...');
			context.setBatchCompletionMessage(null);
		});

		const fileList = services.getCurrentFileList();
		yield* Effect.sync(() => services.console.log('Current file list:', fileList));
		if (!fileList?.files?.length) {
			yield* Effect.sync(() => {
				services.console.log('StatusPanel: No files found');
				services.feedback.showError('No audio files selected. Please add files to process.');
			});
			return;
		}

		if (fileList.validCount === 0) {
			yield* Effect.sync(() => {
				services.console.log('StatusPanel: No valid files found');
				services.feedback.showError(
					'No valid audio files found. Please check your files and try again.',
				);
			});
			return;
		}

		yield* Effect.sync(() =>
			services.console.log('StatusPanel: Files validated, getting output configuration...'),
		);

		const processingRequestConfig = yield* readProcessingConfig(services);
		if (!processingRequestConfig) {
			return;
		}

		yield* Effect.sync(() =>
			services.console.log(
				'StatusPanel: Processing request configuration retrieved:',
				processingRequestConfig,
			),
		);

		const filePaths = validInputFilePaths(fileList);
		const metadataReady = yield* stagePendingMetadataIntent(services, fileList, workflowPromise);
		if (!metadataReady) {
			return;
		}

		const jobType = services.getJobType();
		yield* Effect.sync(() => context.setCurrentWorkKind(jobType));

		const processPayload = buildProcessPayload(filePaths, processingRequestConfig, jobType);
		yield* ensureBatchMetadataLoaded(services, processPayload, workflowPromise);

		const metadataIntentByPath = buildMetadataIntentByPath(services, processPayload);
		const reviewResult = yield* reviewOutputPlan(
			services,
			{
				payload: processPayload,
				metadataIntentByPath,
				previewSeconds: options?.previewSeconds,
			},
			workflowPromise,
		);
		if (reviewResult.status === 'blocked') {
			yield* Effect.sync(() => services.feedback.showError(reviewResult.message));
			return;
		}
		if (reviewResult.status === 'cancelled') {
			return;
		}

		yield* beginProcessingExecution(services, context);
		yield* startProcessingRuntime(context);

		const result = yield* processingCommand(services, {
			payload: reviewResult.payload,
			metadataIntentByPath,
			previewSeconds: options?.previewSeconds,
		});

		yield* completeProcessingExecution(services, context, result, filePaths);
	}).pipe(
		Effect.catchAll((error) =>
			Effect.gen(function* () {
				const services = yield* ProcessingWorkflowServicesTag;
				yield* handleWorkflowError(services, context, error);
			}),
		),
		Effect.ensuring(
			Effect.gen(function* () {
				const services = yield* ProcessingWorkflowServicesTag;
				yield* Effect.sync(() => services.updateOutputPath('final'));
			}),
		),
	);
}

export function startProcessing(
	context: ProcessingWorkflowContext,
	options?: {
		previewSeconds?: number;
	},
	layer?: ProcessingWorkflowLayer,
): Promise<void> {
	return (async () => {
		const workflowLayer =
			layer ?? (await import('./processingWorkflowLive')).ProcessingWorkflowLive;
		return runAppEffect(
			processingWorkflowProgram(context, options).pipe(Effect.provide(workflowLayer)),
		);
	})();
}
