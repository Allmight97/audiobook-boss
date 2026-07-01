import {
	Effect,
	type AppEffect,
	type AppLayer,
	makeWorkflowKit,
	runAppEffect,
} from '../../lib/effect/appEffect';
import { tauriClient } from '../../lib/tauri/client';
import type {
	CollisionPolicy,
	OutputKind,
	ProcessPayload,
	ProcessingPreflightPlan,
} from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { openCollisionDialog } from '../collisionDialog/state.svelte';
import {
	buildOutputPathPreviewContext,
	readOutputPathPreviewMetadataDraft,
	type OutputPathPreviewMetadataDraft,
	showOutputError,
	updateMetadataIntentWarnings,
} from './preview';
import {
	beginOutputPreviewRequest,
	getOutputNamingConfig,
	getState,
	isLatestOutputPreviewRequest,
	setOutputPreview,
} from './state.svelte';

export interface OutputPlanWorkflowServices {
	getState: typeof getState;
	readOutputPathPreviewMetadataDraft: () => OutputPathPreviewMetadataDraft;
	updateMetadataIntentWarnings: typeof updateMetadataIntentWarnings;
	buildOutputPathPreviewContext: typeof buildOutputPathPreviewContext;
	beginOutputPreviewRequest: typeof beginOutputPreviewRequest;
	isLatestOutputPreviewRequest: typeof isLatestOutputPreviewRequest;
	getOutputNamingConfig: typeof getOutputNamingConfig;
	setOutputPreview: typeof setOutputPreview;
	showOutputError: typeof showOutputError;
	previewOutputPath: typeof tauriClient.previewOutputPath;
	preflightProcessingPlan: typeof tauriClient.preflightProcessingPlan;
	openCollisionDialog: (plan: ProcessingPreflightPlan) => Promise<CollisionPolicy | null>;
	console: Pick<Console, 'error'>;
}

export type MetadataIntentByPath = Record<string, MetadataIntentPatch>;

export interface OutputPlanReviewRequest {
	payload: ProcessPayload;
	metadataIntentByPath: MetadataIntentByPath | null;
	previewSeconds?: number | null;
}

export type OutputPlanReviewResult =
	| { status: 'approved'; payload: ProcessPayload; plan: ProcessingPreflightPlan }
	| { status: 'blocked'; message: string; plan: ProcessingPreflightPlan }
	| { status: 'cancelled' };

export type OutputPlanWorkflowAction =
	| { type: 'previewOutputPath'; outputKind: OutputKind }
	| { type: 'reviewForProcessing'; request: OutputPlanReviewRequest };

export type OutputPlanWorkflowServicesId = 'OutputPanel/OutputPlanWorkflowServices';
export type OutputPlanWorkflowLayer = AppLayer<OutputPlanWorkflowServicesId>;

const kit = makeWorkflowKit(
	'OutputPanel/OutputPlanWorkflowServices',
	'OutputPlanWorkflowFailed',
)<OutputPlanWorkflowServices>();

export const OutputPlanWorkflowServicesTag = kit.Tag;

export function makeOutputPlanWorkflowServicesLayer(
	services: OutputPlanWorkflowServices,
): OutputPlanWorkflowLayer {
	return kit.makeLive(services);
}

const liveOutputPlanWorkflowServices = {
	getState,
	readOutputPathPreviewMetadataDraft,
	updateMetadataIntentWarnings,
	buildOutputPathPreviewContext,
	beginOutputPreviewRequest,
	isLatestOutputPreviewRequest,
	getOutputNamingConfig,
	setOutputPreview,
	showOutputError,
	previewOutputPath: tauriClient.previewOutputPath,
	preflightProcessingPlan: tauriClient.preflightProcessingPlan,
	openCollisionDialog,
	console,
} satisfies OutputPlanWorkflowServices;

export const OutputPlanWorkflowLive = makeOutputPlanWorkflowServicesLayer(
	liveOutputPlanWorkflowServices,
);

export const OutputPlanWorkflowFailed = kit.Failed;
export type OutputPlanWorkflowFailed = InstanceType<typeof kit.Failed>;

const workflowPromise = kit.tryPromise;

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

export function outputPathPreviewBody(
	outputKind: Parameters<OutputPlanWorkflowServices['previewOutputPath']>[0]['outputKind'],
	reservedRequestId?: number,
): AppEffect<void, OutputPlanWorkflowFailed, OutputPlanWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* OutputPlanWorkflowServicesTag;
		const state = services.getState();
		const previewMetadataDraft = services.readOutputPathPreviewMetadataDraft();

		yield* workflowPromise(
			() => services.updateMetadataIntentWarnings(previewMetadataDraft),
			'Failed to validate metadata intent for output preview.',
		).pipe(
			Effect.catchAll((error) =>
				Effect.sync(() => {
					services.console.error('Metadata preview validation failed:', error.cause);
					services.showOutputError('Failed to validate metadata preview.');
				}),
			),
		);

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

export function outputPlanReviewBody(
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

export async function runOutputPathPreviewWorkflow(
	outputKind: Parameters<OutputPlanWorkflowServices['previewOutputPath']>[0]['outputKind'],
	layer?: OutputPlanWorkflowLayer,
	reservedRequestId?: number,
): Promise<void> {
	const workflowLayer = layer ?? OutputPlanWorkflowLive;
	return runAppEffect(
		outputPathPreviewBody(outputKind, reservedRequestId).pipe(Effect.provide(workflowLayer)),
	);
}

export async function runOutputPlanReviewWorkflow(
	request: OutputPlanReviewRequest,
	layer?: OutputPlanWorkflowLayer,
): Promise<OutputPlanReviewResult> {
	const workflowLayer = layer ?? OutputPlanWorkflowLive;
	return runAppEffect(outputPlanReviewBody(request).pipe(Effect.provide(workflowLayer)));
}
