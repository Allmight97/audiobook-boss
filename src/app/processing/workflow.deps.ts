import { tauriClient } from '../../lib/tauri/client';
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
	readonly encoding: Pick<EncodingOwner, 'audioRequest'>;
	readonly output: Pick<OutputPlanOwner, 'readRequestConfig' | 'openCollisionReview'>;
	readonly remoteSource: Pick<RemoteSourceOwner, 'processingAssets' | 'withSubmissionRetention'>;
	readonly showError: (message: string) => void;
};

export function makeProcessingWorkflowLive(deps: ProcessingWorkflowLiveDeps) {
	const services: ProcessingWorkflowServices = {
		currentTitles: () => deps.input.view().files,
		sourcesFor: (file) => deps.input.sourcesFor(file),
		readProcessingRequestConfig: (titles) => {
			if (titles.some((file) => deps.input.audioChoiceRequired(file)))
				throw new Error('Choose audio handling for each grouped title before processing.');
			if (titles.some((title) => deps.input.sourcesFor(title).some((source) => !source.isValid)))
				throw new Error('Remove or replace invalid source files before processing.');
			return {
				audioRequests: titles.map((file) => deps.encoding.audioRequest(file)),
				...deps.output.readRequestConfig(),
			};
		},
		stageMetadata: () => deps.metadata.stageCurrentSelection(),
		intentsForProcess: (filePaths) => deps.metadata.intentsForProcess(filePaths),
		setJobControlsEnabled: (enabled) => {
			deps.settings.setControlsEnabled(enabled);
		},
		setFileOrderLocked: (locked) => {
			deps.input.setOrderLocked(locked);
		},
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
