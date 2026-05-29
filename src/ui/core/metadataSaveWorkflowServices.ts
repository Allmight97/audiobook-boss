import type { tauriClient } from '../../lib/tauri/client';
import {
	type AppLayer,
	makeWorkflowLayer,
	makeWorkflowServiceTag,
} from '../../lib/effect/appEffect';
import type { persistPendingMetadataDraftsForCurrentSelection } from '../fileList/metadataStaging';
import type { getCurrentFileList } from '../fileList/state.svelte';
import type { resetDirtyState } from '../metadataForm';
import type {
	clearPendingMetadataForFile,
	getPendingMetadataIntentEntries,
} from '../metadataState';
import type {
	beginMetadataSaveInStatusPanel,
	completeMetadataSaveInStatusPanel,
	failMetadataSaveInStatusPanel,
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
} from '../statusPanel';

export interface MetadataSaveWorkflowServices {
	getCurrentFileList: typeof getCurrentFileList;
	initStatusPanel: typeof initStatusPanel;
	isStatusPanelProcessing: typeof isStatusPanelProcessing;
	pushStatusPanelTransientStatus: typeof pushStatusPanelTransientStatus;
	isMetadataSaveInProgress: () => boolean;
	setMetadataSaveInProgress: (isInProgress: boolean) => void;
	persistPendingMetadataDraftsForCurrentSelection: typeof persistPendingMetadataDraftsForCurrentSelection;
	getPendingMetadataIntentEntries: typeof getPendingMetadataIntentEntries;
	saveMetadataBatch: typeof tauriClient.saveMetadataBatch;
	clearPendingMetadataForFile: typeof clearPendingMetadataForFile;
	resetDirtyState: typeof resetDirtyState;
	beginMetadataSaveInStatusPanel: typeof beginMetadataSaveInStatusPanel;
	completeMetadataSaveInStatusPanel: typeof completeMetadataSaveInStatusPanel;
	failMetadataSaveInStatusPanel: typeof failMetadataSaveInStatusPanel;
	console: Pick<Console, 'error' | 'log'>;
}

export type MetadataSaveWorkflowServicesId = 'Core/MetadataSaveWorkflowServices';
export type MetadataSaveWorkflowLayer = AppLayer<MetadataSaveWorkflowServicesId>;

export const MetadataSaveWorkflowServicesTag = makeWorkflowServiceTag<
	MetadataSaveWorkflowServicesId,
	MetadataSaveWorkflowServices
>('Core/MetadataSaveWorkflowServices');

export function makeMetadataSaveWorkflowServicesLayer(
	services: MetadataSaveWorkflowServices,
): MetadataSaveWorkflowLayer {
	return makeWorkflowLayer(MetadataSaveWorkflowServicesTag, services);
}
