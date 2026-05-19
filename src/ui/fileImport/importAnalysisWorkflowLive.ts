import { tauriClient } from '../../lib/tauri/client';
import {
	appendFileList,
	persistPendingMetadataDraftsForCurrentSelection,
} from '../fileList/actions';
import { isOrderLocked } from '../fileList/state.svelte';
import { pushStatusPanelTransientStatus } from '../statusPanel';
import { clearFileImportError, setFileImportError } from './state.svelte';
import {
	makeImportAnalysisWorkflowServicesLayer,
	type ImportAnalysisWorkflowServices,
} from './importAnalysisWorkflowServices';

export const liveImportAnalysisWorkflowServices: ImportAnalysisWorkflowServices = {
	isOrderLocked,
	openFiles: tauriClient.openFiles,
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
