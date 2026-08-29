import { tauriClient } from '../../lib/tauri/client';
import { setConcurrencyControlsEnabledAtom } from '../appSettings';
import { inputViewAtom, jobTypeAtom, setOrderLockedAtom } from '../inputSession';
import {
	cacheMetadataForFile,
	collectActionableMetadataIntent,
	commitPreparedMetadataDrafts,
	getMetadataForFile,
	hasDirtyMetadataFields,
	metadataCapabilityAtom,
	metadataEditorAtom,
	prepareMetadataDrafts,
	readMetadataForm,
	readUncachedMetadataSnapshot,
	stageMetadataIntentPatch,
} from '../metadataSession';
import { runOutputPlanReviewWorkflow } from '../outputPlan';
import { fileListFromInput } from './input';
import { processingRegistry } from './registry';
import { makeProcessingWorkflowServicesLayer, type ProcessingWorkflowServices } from './workflow';
import { showError } from './view';
import { openGeneratedPreviewIfSingle } from './preview';
import { readProcessingRequestConfig } from './config';

function liveFileList() {
	return fileListFromInput(processingRegistry().get(inputViewAtom));
}

function liveEditor() {
	return processingRegistry().get(metadataEditorAtom);
}

const liveProcessingWorkflowServices = {
	getCurrentFileList: liveFileList,
	getSelectedFileIndex: () => processingRegistry().get(inputViewAtom).selectedAnchor,
	getSelectedFileIndices: () => new Set(processingRegistry().get(inputViewAtom).selectedIndices),
	readProcessingRequestConfig,
	getJobType: () => processingRegistry().get(jobTypeAtom),
	hasDirtyMetadataFields: () => {
		const editor = liveEditor();
		return hasDirtyMetadataFields(editor.form, editor.cover);
	},
	readMetadataForm: (options) => {
		const editor = liveEditor();
		return readMetadataForm(editor.form, {
			...options,
			coverArtBytes: editor.cover.currentCoverArt,
			coverArtRemovalRequested: editor.cover.coverArtRemovalRequested,
		});
	},
	collectActionableMetadataIntent,
	getMetadataForFile,
	cacheMetadataForFile,
	stageMetadataIntentPatch,
	async stageMetadataToSelection(options?: { showStatus?: boolean }): Promise<boolean> {
		const registry = processingRegistry();
		const view = registry.get(inputViewAtom);
		const editor = liveEditor();
		const capability = registry.get(metadataCapabilityAtom);
		const selectedFiles = view.selectedIndices
			.map((index) => view.files[index])
			.filter((file) => file != null);
		const prepared = await prepareMetadataDrafts({
			form: editor.form,
			cover: editor.cover,
			selectedFiles,
			validate: (patch) => capability.validateMetadataIntentPatch(patch),
			readUncachedMetadata: (file) =>
				readUncachedMetadataSnapshot(file, (path) => capability.readAudioMetadata(path)),
		});
		if (!prepared.ok) {
			if (options?.showStatus) {
				showError(prepared.message);
			}
			return false;
		}
		return commitPreparedMetadataDrafts(prepared.prepared);
	},
	setJobControlsEnabled: (enabled) => {
		processingRegistry().set(setConcurrencyControlsEnabledAtom, enabled);
	},
	setFileOrderLocked: (locked) => {
		processingRegistry().set(setOrderLockedAtom, locked);
	},
	validateMetadataIntentPatch: (patch) =>
		processingRegistry().get(metadataCapabilityAtom).validateMetadataIntentPatch(patch),
	readAudioMetadata: (path) =>
		processingRegistry().get(metadataCapabilityAtom).readAudioMetadata(path),
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
