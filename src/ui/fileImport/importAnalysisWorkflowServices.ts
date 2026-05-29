import type { tauriClient } from '../../lib/tauri/client';
import type { AudioFile } from '../../types/audio';
import {
	type AppLayer,
	makeWorkflowLayer,
	makeWorkflowServiceTag,
} from '../../lib/effect/appEffect';
import type { appendFileList } from '../fileList/actions';
import type { persistPendingMetadataDraftsForCurrentSelection } from '../fileList/metadataStaging';
import type { isOrderLocked } from '../fileList/state.svelte';
import type { pushStatusPanelTransientStatus } from '../statusPanel';
import type { clearFileImportError, setFileImportError } from './state.svelte';

export interface ImportAnalysisWorkflowServices {
	isOrderLocked: typeof isOrderLocked;
	getSupportedAudioImportMetadata: typeof tauriClient.getSupportedAudioImportMetadata;
	openFiles: typeof tauriClient.openFiles;
	openDirectory: typeof tauriClient.openDirectory;
	discoverAudioImportPaths: typeof tauriClient.discoverAudioImportPaths;
	analyzeAudioFiles: typeof tauriClient.analyzeAudioFiles;
	persistPendingMetadataDraftsForCurrentSelection: typeof persistPendingMetadataDraftsForCurrentSelection;
	appendFileList: typeof appendFileList;
	pushStatusPanelTransientStatus: typeof pushStatusPanelTransientStatus;
	setFileImportError: typeof setFileImportError;
	clearFileImportError: typeof clearFileImportError;
	console: Pick<Console, 'error'>;
}

export type ImportAnalysisWorkflowAction =
	| { type: 'clickToSelect'; existingFiles: AudioFile[] }
	| { type: 'clickToSelectFolder'; existingFiles: AudioFile[] }
	| { type: 'importPaths'; paths: string[]; existingFiles: AudioFile[] };

export type ImportAnalysisWorkflowServicesId = 'FileImport/ImportAnalysisWorkflowServices';
export type ImportAnalysisWorkflowLayer = AppLayer<ImportAnalysisWorkflowServicesId>;

export const ImportAnalysisWorkflowServicesTag = makeWorkflowServiceTag<
	ImportAnalysisWorkflowServicesId,
	ImportAnalysisWorkflowServices
>('FileImport/ImportAnalysisWorkflowServices');

export function makeImportAnalysisWorkflowServicesLayer(
	services: ImportAnalysisWorkflowServices,
): ImportAnalysisWorkflowLayer {
	return makeWorkflowLayer(ImportAnalysisWorkflowServicesTag, services);
}
