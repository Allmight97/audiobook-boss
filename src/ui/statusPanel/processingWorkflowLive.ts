import { tauriClient } from '../../lib/tauri/client';
import {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	setFileOrderLocked,
	stageMetadataToSelection,
} from '../fileList';
import { getJobType, setJobControlsEnabled } from '../jobControls';
import { hasDirtyMetadataFields, readMetadataForm } from '../metadataForm';
import {
	getAllMetadataIntentPatches,
	getMetadataForFile,
	getMetadataIntentPatchForFile,
	setMetadataForFile,
} from '../metadataState';
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
	setJobControlsEnabled,
	setFileOrderLocked,
	validateMetadataIntentPatch: tauriClient.validateMetadataIntentPatch,
	readAudioMetadata: tauriClient.readAudioMetadata,
	processAudiobookFiles: tauriClient.processAudiobookFiles,
	submitProcessingOperation: tauriClient.submitProcessingOperation,
	runOutputPlanReviewWorkflow,
	openGeneratedPreviewIfSingle,
	feedback,
	console,
};

export const ProcessingWorkflowLive = makeProcessingWorkflowServicesLayer(
	liveProcessingWorkflowServices,
);
