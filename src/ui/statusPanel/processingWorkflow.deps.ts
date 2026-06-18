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
import { runOutputPlanReviewWorkflow, updateOutputPath } from '../outputPanel';
import {
	makeProcessingWorkflowServicesLayer,
	type ProcessingWorkflowServices,
} from './processingWorkflow';
import { showError } from './viewState.svelte';
import { openGeneratedPreviewIfSingle } from './preview';
import { readProcessingRequestConfig } from './processingConfig';

const liveProcessingWorkflowServices = {
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
	feedback: { showError },
	console,
} satisfies ProcessingWorkflowServices;

export const ProcessingWorkflowLive = makeProcessingWorkflowServicesLayer(
	liveProcessingWorkflowServices,
);
