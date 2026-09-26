import { Effect, type AppEffect } from '../../lib/effect/appEffect';
import type {
	JobType,
	ProcessPayload,
	ProcessingRequestConfig,
	SupplementalProcessingAsset,
} from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import type { OutputPlanReviewResult } from '../outputPlan';
import type { ProcessingWorkflowFailed } from './workflow';
import type { ProcessingWorkflowServices } from './workflow';

type MetadataIntentByPath = Record<string, MetadataIntentPatch>;

type ProcessingWorkflowPromise = <A>(
	evaluate: () => PromiseLike<A>,
	message: string,
) => AppEffect<A, ProcessingWorkflowFailed>;

function toWireInputIds(inputIds: readonly (string | undefined)[]): (string | null)[] {
	return inputIds.map((inputId) => inputId ?? null);
}

export function buildProcessPayload(
	filePaths: string[],
	inputIds: (string | undefined)[],
	processingRequestConfig: ProcessingRequestConfig,
	jobType: JobType,
	supplementalAssetsByInputId?: Record<string, SupplementalProcessingAsset[]>,
): ProcessPayload {
	return {
		inputFiles: filePaths,
		audioRequests: processingRequestConfig.audioRequests,
		inputIds: toWireInputIds(inputIds),
		outputDir: processingRequestConfig.outputDirectory,
		jobType,
		outputNaming: processingRequestConfig.outputNaming,
		supplementalAssetsByInputId,
	};
}

const STAGE_FAILURE_MESSAGES = {
	stale: 'Metadata changed while preparing to process. Start processing again.',
	noTarget: 'Select a valid input file before processing metadata edits.',
} as const;

export function stagePendingMetadataIntent(
	services: ProcessingWorkflowServices,
	workflowPromise: ProcessingWorkflowPromise,
): AppEffect<boolean, ProcessingWorkflowFailed> {
	return Effect.gen(function* () {
		const outcome = yield* workflowPromise(
			() => services.stageMetadata(),
			'Failed to stage metadata for processing.',
		);
		if (outcome.status === 'staged') {
			return true;
		}
		const message =
			outcome.status === 'invalid' ? outcome.message : STAGE_FAILURE_MESSAGES[outcome.status];
		yield* Effect.sync(() => services.feedback.showError(message));
		return false;
	});
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
