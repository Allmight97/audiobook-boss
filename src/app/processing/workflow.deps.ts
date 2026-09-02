import { tauriClient } from '../../lib/tauri/client';
import { boundProcessingInput, boundProcessingMetadata, boundProcessingSettings } from './bind';
import { fileListFromInput } from './input';
import {
	cacheMetadataForFile,
	collectActionableMetadataIntent,
	getMetadataForFile,
	stageMetadataIntentPatch,
} from '../metadataSession';
import { runOutputPlanReviewWorkflow } from '../outputPlan';
import { makeProcessingWorkflowServicesLayer, type ProcessingWorkflowServices } from './workflow';
import { showError } from './view';
import { openGeneratedPreviewIfSingle } from './preview';
import { readProcessingRequestConfig } from './config';
import type { RemoteSourceOwner } from '../remoteSource';

function liveFileList() {
	const view = boundProcessingInput()?.view();
	return view ? fileListFromInput(view) : null;
}

function liveMetadata() {
	return boundProcessingMetadata();
}

export function makeProcessingWorkflowLive(
	remoteSource: Pick<RemoteSourceOwner, 'processingAssets' | 'withSubmissionRetention'>,
) {
	const services = {
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
			const metadata = liveMetadata();
			if (!metadata) {
				return false;
			}
			const staged = await metadata.stageCurrentSelectionForProcess();
			if (!staged && options?.showStatus) {
				showError('Fix metadata validation errors before processing.');
			}
			return staged;
		},
		setJobControlsEnabled: (enabled) => {
			boundProcessingSettings()?.setControlsEnabled(enabled);
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
		remoteSource,
		runOutputPlanReviewWorkflow,
		openGeneratedPreviewIfSingle,
		feedback: { showError },
		console,
	} satisfies ProcessingWorkflowServices;

	return makeProcessingWorkflowServicesLayer(services);
}
