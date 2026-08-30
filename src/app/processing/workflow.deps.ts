import { tauriClient } from '../../lib/tauri/client';
import { setConcurrencyControlsEnabledAtom } from '../appSettings';
import { boundProcessingInput, boundProcessingMetadata } from './bind';
import { fileListFromInput } from './input';
import {
	cacheMetadataForFile,
	collectActionableMetadataIntent,
	commitPreparedMetadataDrafts,
	getMetadataForFile,
	prepareMetadataDrafts,
	readUncachedMetadataSnapshot,
	stageMetadataIntentPatch,
} from '../metadataSession';
import { runOutputPlanReviewWorkflow } from '../outputPlan';
import { processingRegistry } from './registry';
import { makeProcessingWorkflowServicesLayer, type ProcessingWorkflowServices } from './workflow';
import { showError } from './view';
import { openGeneratedPreviewIfSingle } from './preview';
import { readProcessingRequestConfig } from './config';

function liveFileList() {
	const view = boundProcessingInput()?.view();
	return view ? fileListFromInput(view) : null;
}

function liveMetadata() {
	return boundProcessingMetadata();
}

const liveProcessingWorkflowServices = {
	getCurrentFileList: liveFileList,
	getSelectedFileIndex: () => boundProcessingInput()?.view().selectedAnchor ?? -1,
	getSelectedFileIndices: () => new Set(boundProcessingInput()?.view().selectedIndices ?? []),
	readProcessingRequestConfig,
	getJobType: () => boundProcessingInput()?.jobType() ?? 'batch',
	hasDirtyMetadataFields: () => liveMetadata()?.readHasDirtyMetadata() ?? false,
	readMetadataForm: () => liveMetadata()?.readMetadata() ?? {},
	collectActionableMetadataIntent,
	getMetadataForFile,
	cacheMetadataForFile,
	stageMetadataIntentPatch,
	async stageMetadataToSelection(options?: { showStatus?: boolean }): Promise<boolean> {
		const view = boundProcessingInput()?.view();
		const metadata = liveMetadata();
		if (!view || !metadata) {
			return false;
		}
		const current = metadata.view();
		const capability = metadata.capability();
		const selectedFiles = view.selectedIndices
			.map((index) => view.files[index])
			.filter((file) => file != null);
		const prepared = await prepareMetadataDrafts({
			form: current.form,
			cover: current.cover,
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
		boundProcessingInput()?.setOrderLocked(locked);
	},
	validateMetadataIntentPatch: (patch) => {
		const metadata = liveMetadata();
		if (!metadata) {
			return Promise.reject(new Error('Metadata owner is not mounted'));
		}
		return metadata.capability().validateMetadataIntentPatch(patch);
	},
	readAudioMetadata: (path) => {
		const metadata = liveMetadata();
		if (!metadata) {
			return Promise.reject(new Error('Metadata owner is not mounted'));
		}
		return metadata.capability().readAudioMetadata(path);
	},
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
