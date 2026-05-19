import {
	Data,
	Effect,
	type AppEffect,
	runAppEffect,
	workflowTryPromise,
} from '../../lib/effect/appEffect';
import type { CollisionPolicy, ProcessPayload, ProcessingPreflightPlan } from '../../types/audio';
import {
	OutputPlanWorkflowServicesTag,
	type OutputPlanReviewRequest,
	type OutputPlanReviewResult,
	type OutputPlanWorkflowLayer,
	type OutputPlanWorkflowServices,
	type OutputPlanWorkflowServicesId,
} from './outputPlanWorkflowServices';

export {
	OutputPlanWorkflowServicesTag,
	makeOutputPlanWorkflowServicesLayer,
	type OutputPlanReviewRequest,
	type OutputPlanReviewResult,
	type OutputPlanWorkflowAction,
	type OutputPlanWorkflowLayer,
	type OutputPlanWorkflowServices,
	type OutputPlanWorkflowServicesId,
} from './outputPlanWorkflowServices';

export class OutputPlanWorkflowFailed extends Data.TaggedError('OutputPlanWorkflowFailed')<{
	readonly message: string;
	readonly cause: unknown;
}> {}

function workflowFailure(message: string, cause: unknown): OutputPlanWorkflowFailed {
	return new OutputPlanWorkflowFailed({ message, cause });
}

function workflowPromise<A>(
	evaluate: () => PromiseLike<A>,
	message: string,
): AppEffect<A, OutputPlanWorkflowFailed> {
	return workflowTryPromise(evaluate, message, workflowFailure);
}

function getBlockingReviewMessage(plan: ProcessingPreflightPlan): string | null {
	const blocked = plan.outputs.find((output) => output.review?.canProceed === false);
	return blocked?.review?.message ?? null;
}

function approvePayload(payload: ProcessPayload, plan: ProcessingPreflightPlan): ProcessPayload {
	return {
		...payload,
		collisionPolicy: plan.collisionPolicy,
		preflightSignature: plan.planSignature,
	};
}

function outputPathPreviewBody(
	outputKind: Parameters<OutputPlanWorkflowServices['previewOutputPath']>[0]['outputKind'],
	reservedRequestId?: number,
): AppEffect<void, OutputPlanWorkflowFailed, OutputPlanWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* OutputPlanWorkflowServicesTag;
		const state = services.getState();
		const previewMetadataDraft = services.readOutputPathPreviewMetadataDraft();

		services.updateSeriesPartWarning(previewMetadataDraft);
		services.updateSubseriesPartWarning(previewMetadataDraft);

		if (!state.outputDirectory) {
			services.setOutputPreview('Select output directory...', 'No directory selected');
			return;
		}

		const previewContext = services.buildOutputPathPreviewContext();
		const requestId = reservedRequestId ?? services.beginOutputPreviewRequest();

		const previewPath = yield* workflowPromise(
			() =>
				services.previewOutputPath({
					outputDir: previewContext.outputDirectory,
					metadata: previewMetadataDraft,
					outputNaming: services.getOutputNamingConfig(),
					sourcePath: previewContext.sourcePath,
					outputKind,
				}),
			'Output preview failed.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					if (!services.isLatestOutputPreviewRequest(requestId)) {
						return null;
					}
					const message = 'Output preview unavailable. Fix metadata/template and retry.';
					services.setOutputPreview(message);
					services.showOutputError(`Rust preview failed: ${String(error.cause)}`);
					return null;
				}),
			),
		);

		if (previewPath == null || !services.isLatestOutputPreviewRequest(requestId)) {
			return;
		}

		services.setOutputPreview(previewPath);
	});
}

function outputPlanReviewBody(
	request: OutputPlanReviewRequest,
): AppEffect<OutputPlanReviewResult, OutputPlanWorkflowFailed, OutputPlanWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* OutputPlanWorkflowServicesTag;
		const { payload, metadataIntentByPath, previewSeconds } = request;

		const initialPlan = yield* workflowPromise(
			() =>
				services.preflightProcessingPlan({
					payload,
					metadataIntent: metadataIntentByPath,
					previewSeconds,
				}),
			'Output plan preflight failed.',
		);
		const hardBlockMessage = getBlockingReviewMessage(initialPlan);
		if (hardBlockMessage) {
			return { status: 'blocked' as const, message: hardBlockMessage, plan: initialPlan };
		}

		const needsReview = initialPlan.outputs.some((output) => output.action === 'review_required');
		if (!needsReview) {
			return {
				status: 'approved' as const,
				payload: approvePayload(payload, initialPlan),
				plan: initialPlan,
			};
		}

		const selectedPolicy = yield* workflowPromise(
			() => services.openCollisionDialog(initialPlan),
			'Output collision review failed.',
		);
		if (!selectedPolicy) {
			return { status: 'cancelled' as const };
		}

		const reviewedPayload: ProcessPayload = {
			...payload,
			collisionPolicy: selectedPolicy as CollisionPolicy,
		};
		const reviewedPlan = yield* workflowPromise(
			() =>
				services.preflightProcessingPlan({
					payload: reviewedPayload,
					metadataIntent: metadataIntentByPath,
					previewSeconds,
				}),
			'Reviewed output plan preflight failed.',
		);
		const reviewedHardBlock = getBlockingReviewMessage(reviewedPlan);
		if (reviewedHardBlock) {
			return { status: 'blocked' as const, message: reviewedHardBlock, plan: reviewedPlan };
		}

		return {
			status: 'approved' as const,
			payload: {
				...reviewedPayload,
				preflightSignature: reviewedPlan.planSignature,
			},
			plan: reviewedPlan,
		};
	});
}

async function defaultOutputPlanWorkflowLayer(): Promise<OutputPlanWorkflowLayer> {
	const live = await import('./outputPlanWorkflowLive');
	return live.OutputPlanWorkflowLive;
}

export function outputPathPreviewWorkflowExecution(
	outputKind: Parameters<OutputPlanWorkflowServices['previewOutputPath']>[0]['outputKind'],
	reservedRequestId?: number,
): AppEffect<void, OutputPlanWorkflowFailed, OutputPlanWorkflowServicesId> {
	return outputPathPreviewBody(outputKind, reservedRequestId);
}

export function outputPlanReviewWorkflowExecution(
	request: OutputPlanReviewRequest,
): AppEffect<OutputPlanReviewResult, OutputPlanWorkflowFailed, OutputPlanWorkflowServicesId> {
	return outputPlanReviewBody(request);
}

export async function runOutputPathPreviewWorkflow(
	outputKind: Parameters<OutputPlanWorkflowServices['previewOutputPath']>[0]['outputKind'],
	layer?: OutputPlanWorkflowLayer,
	reservedRequestId?: number,
): Promise<void> {
	const workflowLayer = layer ?? (await defaultOutputPlanWorkflowLayer());
	return runAppEffect(
		outputPathPreviewBody(outputKind, reservedRequestId).pipe(Effect.provide(workflowLayer)),
	);
}

export async function runOutputPlanReviewWorkflow(
	request: OutputPlanReviewRequest,
	layer?: OutputPlanWorkflowLayer,
): Promise<OutputPlanReviewResult> {
	const workflowLayer = layer ?? (await defaultOutputPlanWorkflowLayer());
	return runAppEffect(outputPlanReviewBody(request).pipe(Effect.provide(workflowLayer)));
}
