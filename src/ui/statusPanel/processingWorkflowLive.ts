import { tauriClient } from '../../lib/tauri/client';
import { setFileOrderLocked, stageMetadataToSelection } from '../fileList/actions';
import {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
} from '../fileList/state.svelte';
import { getJobType, setJobControlsEnabled } from '../jobControls';
import { hasDirtyMetadataFields, readMetadataForm } from '../metadataForm';
import {
	getAllMetadataIntentPatches,
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	setMetadataForFile,
} from '../metadataState';
import {
	getSeriesPartValidationError,
	getSubseriesPartValidationError,
} from '../metadataValidation';
import { updateOutputPath } from '../outputPanel';
import { runOutputPlanReviewWorkflow } from '../outputPanel/outputPlanWorkflow';
import * as feedback from './feedback';
import { openGeneratedPreviewIfSingle } from './preview';
import { readProcessingRequestConfig } from './processingConfig';
import {
	makeProcessingWorkflowServicesLayer,
	type ProcessingWorkflowServices,
} from './processingWorkflowServices';

const liveProcessingWorkflowServices: ProcessingWorkflowServices = {
	updateOutputPath,
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	readProcessingRequestConfig,
	getJobType,
	hasDirtyMetadataFields,
	readMetadataForm,
	getAllMetadataIntentPatches,
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	setMetadataForFile,
	stageMetadataToSelection,
	getSeriesPartValidationError,
	getSubseriesPartValidationError,
	setJobControlsEnabled,
	setFileOrderLocked,
	readAudioMetadata: tauriClient.readAudioMetadata,
	processAudiobookFiles: tauriClient.processAudiobookFiles,
	runOutputPlanReviewWorkflow,
	openGeneratedPreviewIfSingle,
	feedback,
	console,
};

export const ProcessingWorkflowLive = makeProcessingWorkflowServicesLayer(
	liveProcessingWorkflowServices,
);
