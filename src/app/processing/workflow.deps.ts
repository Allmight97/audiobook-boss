import { tauriClient } from '../../lib/tauri/client';
import { fileListFromInput } from './input';
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
		getAudioHandling: (file) => deps.input.audioHandling(file),
		sourcesFor: (file) => deps.input.sourcesFor(file),
		readProcessingRequestConfig: (audioHandling) => {
			if (deps.input.view().files.some((file) => deps.input.audioChoiceRequired(file)))
				throw new Error('Choose audio handling for each grouped title before processing.');
			if (deps.input.view().sourceFiles.some((file) => !file.isValid))
				throw new Error('Remove or replace invalid source files before processing.');
			return {
				...(audioHandling.includes('encode') ? deps.encoding.request() : {}),
				...deps.output.readRequestConfig(),
			};
		},
		hasDirtyMetadataFields: () => deps.metadata.readHasDirtyMetadata(),
		readMetadataForm: () => deps.metadata.readMetadata(),
		stageIntent: (filePath, patch) => deps.metadata.stageIntent(filePath, patch),
		intentsForProcess: (filePaths) => deps.metadata.intentsForProcess(filePaths),
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
