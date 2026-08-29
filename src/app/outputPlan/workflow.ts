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
	OutputNamingConfig,
	ProcessPayload,
	ProcessingPreflightPlan,
} from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { validateMetadataDraft } from '../metadataSession';
import { openCollisionDialog } from './collision';
import type { OutputPathPreviewMetadataDraft } from './types';
import { EMPTY_PREVIEW_TEXT, EMPTY_PREVIEW_TITLE } from './types';

export type OutputPathPreviewContext = {
	readonly outputDirectory: string;
	readonly sourcePath?: string;
	readonly outputNaming: OutputNamingConfig;
	readonly metadataDraft: OutputPathPreviewMetadataDraft;
};

export interface OutputPlanWorkflowServices {
	updateMetadataIntentWarnings: (metadata: OutputPathPreviewMetadataDraft) => Promise<void>;
	beginOutputPreviewRequest: () => number;
	isLatestOutputPreviewRequest: (requestId: number) => boolean;
	setOutputPreview: (text: string, title?: string) => void;
	showOutputError: (message: string) => void;
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

export type OutputPlanWorkflowServicesId = 'OutputPlan/OutputPlanWorkflowServices';
export type OutputPlanWorkflowLayer = AppLayer<OutputPlanWorkflowServicesId>;

const kit = makeWorkflowKit(
	'OutputPlan/OutputPlanWorkflowServices',
	'OutputPlanWorkflowFailed',
)<OutputPlanWorkflowServices>();

export const OutputPlanWorkflowServicesTag = kit.Tag;

export function makeOutputPlanWorkflowServicesLayer(
	services: OutputPlanWorkflowServices,
): OutputPlanWorkflowLayer {
	return kit.makeLive(services);
}

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
	outputKind: OutputKind,
	context: OutputPathPreviewContext,
	reservedRequestId?: number,
): AppEffect<void, OutputPlanWorkflowFailed, OutputPlanWorkflowServicesId> {
	return Effect.gen(function* () {
		const services = yield* OutputPlanWorkflowServicesTag;

		yield* workflowPromise(
			() => services.updateMetadataIntentWarnings(context.metadataDraft),
			'Failed to validate metadata intent for output preview.',
		).pipe(
			Effect.catch((error) =>
				Effect.sync(() => {
					services.console.error('Metadata preview validation failed:', error.cause);
					services.showOutputError('Failed to validate metadata preview.');
				}),
			),
		);

		if (!context.outputDirectory) {
			services.setOutputPreview(EMPTY_PREVIEW_TEXT, EMPTY_PREVIEW_TITLE);
			return;
		}

		const requestId = reservedRequestId ?? services.beginOutputPreviewRequest();
		const previewPath = yield* workflowPromise(
			() =>
				services.previewOutputPath({
					outputDir: context.outputDirectory,
					metadata: context.metadataDraft,
					outputNaming: context.outputNaming,
					sourcePath: context.sourcePath,
					outputKind,
				}),
			'Output preview failed.',
		).pipe(
			Effect.catch((error) =>
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

export async function updateMetadataIntentWarnings(
	metadata: OutputPathPreviewMetadataDraft,
): Promise<void> {
	await validateMetadataDraft(metadata, tauriClient.validateMetadataIntentPatch);
}

export function showOutputError(message: string): void {
	console.error('Output Plan Error:', message);
}

export async function runOutputPathPreviewWorkflow(
	outputKind: OutputKind,
	context: OutputPathPreviewContext,
	layer?: OutputPlanWorkflowLayer,
	reservedRequestId?: number,
): Promise<void> {
	const workflowLayer = layer ?? OutputPlanWorkflowLive;
	return runAppEffect(
		outputPathPreviewBody(outputKind, context, reservedRequestId).pipe(
			Effect.provide(workflowLayer),
		),
	);
}

export async function runOutputPlanReviewWorkflow(
	request: OutputPlanReviewRequest,
	layer?: OutputPlanWorkflowLayer,
): Promise<OutputPlanReviewResult> {
	const workflowLayer = layer ?? OutputPlanWorkflowLive;
	return runAppEffect(outputPlanReviewBody(request).pipe(Effect.provide(workflowLayer)));
}

export const OutputPlanWorkflowLive = makeOutputPlanWorkflowServicesLayer({
	updateMetadataIntentWarnings,
	beginOutputPreviewRequest: () => 0,
	isLatestOutputPreviewRequest: () => true,
	setOutputPreview: () => undefined,
	showOutputError,
	previewOutputPath: tauriClient.previewOutputPath,
	preflightProcessingPlan: tauriClient.preflightProcessingPlan,
	openCollisionDialog,
	console,
});
