import type { tauriClient } from '../../lib/tauri/client';
import {
	type AppLayer,
	makeWorkflowLayer,
	makeWorkflowServiceTag,
} from '../../lib/effect/appEffect';
import type { setFileOrderLocked, stageMetadataToSelection } from '../fileList/actions';
import type {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
} from '../fileList/state.svelte';
import type { getJobType, setJobControlsEnabled } from '../jobControls';
import type { hasDirtyMetadataFields, readMetadataForm } from '../metadataForm';
import type {
	getAllMetadataIntentPatches,
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	setMetadataForFile,
} from '../metadataState';
import type {
	getSeriesPartValidationError,
	getSubseriesPartValidationError,
} from '../metadataValidation';
import type { runOutputPlanReviewWorkflow } from '../outputPanel/outputPlanWorkflow';
import type { readProcessingRequestConfig, updateOutputPath } from '../outputPanel';
import type * as feedback from './feedback';
import type { openGeneratedPreviewIfSingle } from './preview';

export interface ProcessingWorkflowServices {
	updateOutputPath: typeof updateOutputPath;
	getCurrentFileList: typeof getCurrentFileList;
	getSelectedFileIndex: typeof getSelectedFileIndex;
	getSelectedFileIndices: typeof getSelectedFileIndices;
	readProcessingRequestConfig: typeof readProcessingRequestConfig;
	getJobType: typeof getJobType;
	hasDirtyMetadataFields: typeof hasDirtyMetadataFields;
	readMetadataForm: typeof readMetadataForm;
	getAllMetadataIntentPatches: typeof getAllMetadataIntentPatches;
	getMetadataForFile: typeof getMetadataForFile;
	getMetadataIntentPatchForFile: typeof getMetadataIntentPatchForFile;
	setMetadataForFile: typeof setMetadataForFile;
	stageMetadataToSelection: typeof stageMetadataToSelection;
	getSeriesPartValidationError: typeof getSeriesPartValidationError;
	getSubseriesPartValidationError: typeof getSubseriesPartValidationError;
	setJobControlsEnabled: typeof setJobControlsEnabled;
	setFileOrderLocked: typeof setFileOrderLocked;
	readAudioMetadata: typeof tauriClient.readAudioMetadata;
	processAudiobookFiles: typeof tauriClient.processAudiobookFiles;
	runOutputPlanReviewWorkflow: typeof runOutputPlanReviewWorkflow;
	openGeneratedPreviewIfSingle: typeof openGeneratedPreviewIfSingle;
	feedback: Pick<typeof feedback, 'showError'>;
	console: Pick<Console, 'error' | 'log' | 'warn'>;
}

export type ProcessingWorkflowServicesId = 'StatusPanel/ProcessingWorkflowServices';
export type ProcessingWorkflowLayer = AppLayer<ProcessingWorkflowServicesId>;

export const ProcessingWorkflowServicesTag = makeWorkflowServiceTag<
	ProcessingWorkflowServicesId,
	ProcessingWorkflowServices
>('StatusPanel/ProcessingWorkflowServices');

export function makeProcessingWorkflowServicesLayer(
	services: ProcessingWorkflowServices,
): ProcessingWorkflowLayer {
	return makeWorkflowLayer(ProcessingWorkflowServicesTag, services);
}
