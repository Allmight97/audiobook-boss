import { tauriClient } from '../../lib/tauri/client';
import { fileListFromInput } from './input';
import {
	cacheMetadataForFile,
	collectActionableMetadataIntent,
	getMetadataForFile,
	stageMetadataIntentPatch,
} from '../metadataSession';
import { runOutputPlanReviewWorkflow, type OutputPlanOwner } from '../outputPlan';
import { makeProcessingWorkflowServicesLayer, type ProcessingWorkflowServices } from './workflow';
import { openGeneratedPreviewIfSingle } from './preview';
import type { EncodingOwner } from '../encoding';
import type { InputOwner } from '../inputSession';
import type { MetadataOwner } from '../metadataSession';
import type { SettingsOwner } from '../appSettings';
import type { RemoteSourceOwner } from '../remoteSource';

export type ProcessingWorkflowLiveDeps = {
	readonly input: InputOwner;
	readonly metadata: MetadataOwner;
	readonly settings: SettingsOwner;
	readonly encoding: Pick<EncodingOwner, 'request'>;
	readonly output: Pick<OutputPlanOwner, 'readRequestConfig' | 'openCollisionReview'>;
	readonly remoteSource: Pick<RemoteSourceOwner, 'processingAssets' | 'withSubmissionRetention'>;
	readonly showError: (message: string) => void;
};

export function makeProcessingWorkflowLive(deps: ProcessingWorkflowLiveDeps) {
	const services: ProcessingWorkflowServices = {
		getCurrentFileList: () => fileListFromInput(deps.input.view()),
		getSelectedFileIndex: () => deps.input.view().selectedAnchor,
		getSelectedFileIndices: () => new Set(deps.input.view().selectedIndices),
		readProcessingRequestConfig: () => ({
			...deps.encoding.request(),
			...deps.output.readRequestConfig(),
		}),
		getJobType: () => deps.input.jobType(),
		hasDirtyMetadataFields: () => deps.metadata.readHasDirtyMetadata(),
		readMetadataForm: () => deps.metadata.readMetadata(),
		collectActionableMetadataIntent,
		getMetadataForFile,
		cacheMetadataForFile,
		stageMetadataIntentPatch,
		async stageMetadataToSelection(options) {
			const staged = await deps.metadata.stageCurrentSelectionForProcess();
			if (!staged && options?.showStatus) {
				deps.showError('Fix metadata validation errors before processing.');
			}
			return staged;
		},
		setJobControlsEnabled: (enabled) => {
			deps.settings.setControlsEnabled(enabled);
		},
		setFileOrderLocked: (locked) => {
			deps.input.setOrderLocked(locked);
		},
		validateMetadataIntentPatch: (patch) =>
			deps.metadata.capability().validateMetadataIntentPatch(patch),
		readAudioMetadata: (path) => deps.metadata.capability().readAudioMetadata(path),
		processAudiobookFiles: tauriClient.processAudiobookFiles,
		submitProcessingOperation: tauriClient.submitProcessingOperation,
		remoteSource: deps.remoteSource,
		runOutputPlanReviewWorkflow: (request) => runOutputPlanReviewWorkflow(request, deps.output),
		openGeneratedPreviewIfSingle,
		feedback: { showError: deps.showError },
		console,
	};

	return makeProcessingWorkflowServicesLayer(services);
}
