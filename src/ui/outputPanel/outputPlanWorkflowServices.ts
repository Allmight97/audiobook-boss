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
import type {
	buildOutputPathPreviewContext,
	OutputPathPreviewMetadataDraft,
	showOutputError,
} from './preview';
import type {
	beginOutputPreviewRequest,
	getOutputNamingConfig,
	getState,
	isLatestOutputPreviewRequest,
	setOutputPreview,
} from './state.svelte';

export interface OutputPlanWorkflowServices {
	getState: typeof getState;
	readOutputPathPreviewMetadataDraft: () => OutputPathPreviewMetadataDraft;
	updateSeriesPartWarning: (metadata: AudiobookMetadata) => void;
	updateSubseriesPartWarning: (metadata: AudiobookMetadata) => void;
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

export const OutputPlanWorkflowServicesTag = makeWorkflowServiceTag<
	OutputPlanWorkflowServicesId,
	OutputPlanWorkflowServices
>('OutputPanel/OutputPlanWorkflowServices');

export function makeOutputPlanWorkflowServicesLayer(
	services: OutputPlanWorkflowServices,
): OutputPlanWorkflowLayer {
	return makeWorkflowLayer(OutputPlanWorkflowServicesTag, services);
}
