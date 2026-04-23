import { tauriClient } from '../../lib/tauri/client';
import type { CollisionPolicy, ProcessPayload, ProcessingPreflightPlan } from '../../types/audio';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { openCollisionDialog } from '../collisionDialog';

export interface OutputPlanReviewRequest {
	payload: ProcessPayload;
	metadataIntent: Record<string, MetadataIntentPatch> | null;
	previewSeconds?: number | null;
}

interface OutputPlanReviewDeps {
	preflightProcessingPlan: typeof tauriClient.preflightProcessingPlan;
	openCollisionDialog: (plan: ProcessingPreflightPlan) => Promise<CollisionPolicy | null>;
}

export type OutputPlanReviewResult =
	| { status: 'approved'; payload: ProcessPayload; plan: ProcessingPreflightPlan }
	| { status: 'blocked'; message: string; plan: ProcessingPreflightPlan }
	| { status: 'cancelled' };

function getHardBlockingCollisionMessage(plan: ProcessingPreflightPlan): string | null {
	const blocked = plan.outputs.find(
		(output) =>
			output.collision?.kind === 'source_destination_overlap' ||
			output.collision?.kind === 'canonical_path_overlap',
	);
	if (!blocked) {
		return null;
	}
	return (
		blocked.collision?.detail ??
		`Output path '${blocked.requestedPath}' targets an input source file. Choose a different destination.`
	);
}

function approvePayload(payload: ProcessPayload, plan: ProcessingPreflightPlan): ProcessPayload {
	return {
		...payload,
		collisionPolicy: plan.collisionPolicy,
		preflightSignature: plan.planSignature,
	};
}

export async function reviewOutputPlanForProcessing(
	request: OutputPlanReviewRequest,
	deps: Partial<OutputPlanReviewDeps> = {},
): Promise<OutputPlanReviewResult> {
	const preflightProcessingPlan =
		deps.preflightProcessingPlan ?? tauriClient.preflightProcessingPlan;
	const openReviewDialog = deps.openCollisionDialog ?? openCollisionDialog;
	const { payload, metadataIntent, previewSeconds } = request;

	const initialPlan = await preflightProcessingPlan({
		payload,
		metadataIntent,
		previewSeconds,
	});
	const hardBlockMessage = getHardBlockingCollisionMessage(initialPlan);
	if (hardBlockMessage) {
		return { status: 'blocked', message: hardBlockMessage, plan: initialPlan };
	}

	const needsReview = initialPlan.outputs.some((output) => output.action === 'review_required');
	if (!needsReview) {
		return { status: 'approved', payload: approvePayload(payload, initialPlan), plan: initialPlan };
	}

	const selectedPolicy = await openReviewDialog(initialPlan);
	if (!selectedPolicy) {
		return { status: 'cancelled' };
	}

	const reviewedPayload: ProcessPayload = {
		...payload,
		collisionPolicy: selectedPolicy,
	};
	const reviewedPlan = await preflightProcessingPlan({
		payload: reviewedPayload,
		metadataIntent,
		previewSeconds,
	});
	const reviewedHardBlock = getHardBlockingCollisionMessage(reviewedPlan);
	if (reviewedHardBlock) {
		return { status: 'blocked', message: reviewedHardBlock, plan: reviewedPlan };
	}

	return {
		status: 'approved',
		payload: {
			...reviewedPayload,
			preflightSignature: reviewedPlan.planSignature,
		},
		plan: reviewedPlan,
	};
}
