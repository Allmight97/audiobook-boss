import {
	Effect,
	type AppEffect,
	type AppLayer,
	makeWorkflowKit,
	runAppEffect,
} from '../../lib/effect/appEffect';
import { tauriClient } from '../../lib/tauri/client';
import { toUserMessage } from '../../lib/tauri/appError';
import type { CollisionPolicy, ProcessPayload, ProcessingPreflightPlan } from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import type { OutputPlanOwner } from './owner';

export interface OutputPlanWorkflowServices {
	preflightProcessingPlan: typeof tauriClient.preflightProcessingPlan;
	openCollisionDialog: (plan: ProcessingPreflightPlan) => Promise<CollisionPolicy | null>;
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
	}).pipe(
		Effect.mapError((error) =>
			kit.failure(toUserMessage(error.cause, { fallback: error.message }), error.cause),
		),
	);
}

type OutputPlanReviewServices =
	| OutputPlanWorkflowLayer
	| Pick<OutputPlanOwner, 'openCollisionReview'>;

function isCollisionReviewOwner(
	services: OutputPlanReviewServices,
): services is Pick<OutputPlanOwner, 'openCollisionReview'> {
	return 'openCollisionReview' in services;
}

export async function runOutputPlanReviewWorkflow(
	request: OutputPlanReviewRequest,
	services: OutputPlanReviewServices,
): Promise<OutputPlanReviewResult> {
	const workflowLayer = isCollisionReviewOwner(services)
		? makeOutputPlanWorkflowServicesLayer({
				preflightProcessingPlan: tauriClient.preflightProcessingPlan,
				openCollisionDialog: (plan) => services.openCollisionReview(plan),
			})
		: services;
	return runAppEffect(outputPlanReviewBody(request).pipe(Effect.provide(workflowLayer)));
}
