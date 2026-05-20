import { tauriClient } from '../../lib/tauri/client';
import { openCollisionDialog } from '../collisionDialog/state.svelte';
import {
	beginOutputPreviewRequest,
	getOutputNamingConfig,
	getState,
	isLatestOutputPreviewRequest,
	setOutputPreview,
} from './state.svelte';
import {
	buildOutputPathPreviewContext,
	readOutputPathPreviewMetadataDraft,
	showOutputError,
	updateSeriesPartWarning,
	updateSubseriesPartWarning,
} from './preview';
import {
	makeOutputPlanWorkflowServicesLayer,
	type OutputPlanWorkflowServices,
} from './outputPlanWorkflowServices';

export const liveOutputPlanWorkflowServices: OutputPlanWorkflowServices = {
	getState,
	readOutputPathPreviewMetadataDraft,
	updateSeriesPartWarning,
	updateSubseriesPartWarning,
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
};

export const OutputPlanWorkflowLive = makeOutputPlanWorkflowServicesLayer(
	liveOutputPlanWorkflowServices,
);
