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
import { validateMetadataDraft, type MetadataDraftValidation } from '../metadataSession';
import type { OutputPlanOwner } from './owner';
import type { OutputPathPreviewMetadataDraft } from './types';
import { EMPTY_PREVIEW_TEXT, EMPTY_PREVIEW_TITLE } from './types';

export type OutputPathPreviewContext = {
	readonly outputDirectory: string;
	readonly sourcePath?: string;
	readonly outputNaming: OutputNamingConfig;
	readonly metadataDraft: OutputPathPreviewMetadataDraft;
};

export type OutputPathPreviewResult =
	| { readonly ok: true; readonly text: string; readonly title: string }
	| { readonly ok: false; readonly text: string; readonly title: string; readonly cause: unknown };

export const PREVIEW_UNAVAILABLE_TEXT =
	'Output preview unavailable. Fix metadata/template and retry.';

export async function computeOutputPathPreview(
	outputKind: OutputKind,
	context: OutputPathPreviewContext,
	previewOutputPath: typeof tauriClient.previewOutputPath,
): Promise<OutputPathPreviewResult> {
	if (!context.outputDirectory) {
		return { ok: true, text: EMPTY_PREVIEW_TEXT, title: EMPTY_PREVIEW_TITLE };
	}
	try {
		const previewPath = await previewOutputPath({
			outputDir: context.outputDirectory,
			metadata: context.metadataDraft,
			outputNaming: context.outputNaming,
			sourcePath: context.sourcePath,
			outputKind,
		});
		return { ok: true, text: previewPath, title: previewPath };
	} catch (cause) {
		return {
			ok: false,
			text: PREVIEW_UNAVAILABLE_TEXT,
			title: PREVIEW_UNAVAILABLE_TEXT,
			cause,
		};
	}
}

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
	});
}

export async function updateMetadataIntentWarnings(
	metadata: OutputPathPreviewMetadataDraft,
	onMetadataValidation?: (validation: MetadataDraftValidation) => void,
): Promise<void> {
	const validation = await validateMetadataDraft(metadata, tauriClient.validateMetadataIntentPatch);
	onMetadataValidation?.(validation);
}

export function showOutputError(message: string): void {
	console.error('Output Plan Error:', message);
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
