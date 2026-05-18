import type { tauriClient } from '../../lib/tauri/client';
import {
	type AppLayer,
	makeWorkflowLayer,
	makeWorkflowServiceTag,
} from '../../lib/effect/appEffect';
import type {
	CollisionPolicy,
	OutputKind,
	ProcessPayload,
	ProcessingPreflightPlan,
} from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import type { buildOutputPreviewCallSiteState, showOutputError } from './preview';
import type {
	beginOutputPreviewRequest,
	getOutputNamingConfig,
	getState,
	isLatestOutputPreviewRequest,
	setOutputPreview,
} from './state.svelte';

export interface OutputPlanWorkflowServices {
	getState: typeof getState;
	getCurrentMetadata: () => AudiobookMetadata;
	updateSeriesPartWarning: (metadata: AudiobookMetadata) => void;
	updateSubseriesPartWarning: (metadata: AudiobookMetadata) => void;
	buildOutputPreviewCallSiteState: typeof buildOutputPreviewCallSiteState;
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

export interface OutputPlanReviewRequest {
	payload: ProcessPayload;
	metadataIntent: Record<string, MetadataIntentPatch> | null;
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

export const OutputPlanWorkflowServicesTag = makeWorkflowServiceTag<
	OutputPlanWorkflowServicesId,
	OutputPlanWorkflowServices
>('OutputPanel/OutputPlanWorkflowServices');

export function makeOutputPlanWorkflowServicesLayer(
	services: OutputPlanWorkflowServices,
): OutputPlanWorkflowLayer {
	return makeWorkflowLayer(OutputPlanWorkflowServicesTag, services);
}
