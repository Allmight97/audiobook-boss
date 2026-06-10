import { tauriClient } from '../../lib/tauri/client';
import {
	appendFileList,
	isOrderLocked,
	persistPendingMetadataDraftsForCurrentSelection,
} from '../fileList';
import { pushStatusPanelTransientStatus } from '../statusPanel';
import { clearFileImportError, setFileImportError } from './state.svelte';
import {
	makeImportAnalysisWorkflowServicesLayer,
	type ImportAnalysisWorkflowServices,
} from './importAnalysisWorkflowServices';

export const liveImportAnalysisWorkflowServices: ImportAnalysisWorkflowServices = {
	isOrderLocked,
	getSupportedAudioImportMetadata: tauriClient.getSupportedAudioImportMetadata,
	openFiles: tauriClient.openFiles,
	openDirectory: tauriClient.openDirectory,
	discoverAudioImportPaths: tauriClient.discoverAudioImportPaths,
	analyzeAudioFiles: tauriClient.analyzeAudioFiles,
	persistPendingMetadataDraftsForCurrentSelection,
	appendFileList,
	pushStatusPanelTransientStatus,
	setFileImportError,
	clearFileImportError,
	console,
};

export const ImportAnalysisWorkflowLive = makeImportAnalysisWorkflowServicesLayer(
	liveImportAnalysisWorkflowServices,
);
