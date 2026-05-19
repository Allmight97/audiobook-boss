import { get } from 'svelte/store';

import { tauriClient } from '../../lib/tauri/client';
import { persistPendingMetadataDraftsForCurrentSelection } from '../fileList/actions';
import { getCurrentFileList } from '../fileList/state.svelte';
import { resetDirtyState } from '../metadataForm';
import { clearPendingMetadataForFile, getPendingMetadataIntentEntries } from '../metadataState';
import { metadataSaveInProgressStore } from '../metadataSaveState';
import {
	beginMetadataSaveInStatusPanel,
	completeMetadataSaveInStatusPanel,
	failMetadataSaveInStatusPanel,
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
} from '../statusPanel';
import {
	makeMetadataSaveWorkflowServicesLayer,
	type MetadataSaveWorkflowServices,
} from './metadataSaveWorkflowServices';

const liveMetadataSaveWorkflowServices: MetadataSaveWorkflowServices = {
	getCurrentFileList,
	initStatusPanel,
	isStatusPanelProcessing,
	pushStatusPanelTransientStatus,
	isMetadataSaveInProgress: () => get(metadataSaveInProgressStore),
	setMetadataSaveInProgress: (isInProgress) => {
		metadataSaveInProgressStore.set(isInProgress);
	},
	persistPendingMetadataDraftsForCurrentSelection,
	getPendingMetadataIntentEntries,
	saveMetadataBatch: tauriClient.saveMetadataBatch,
	clearPendingMetadataForFile,
	resetDirtyState,
	beginMetadataSaveInStatusPanel,
	completeMetadataSaveInStatusPanel,
	failMetadataSaveInStatusPanel,
	console,
};

export const MetadataSaveWorkflowLive = makeMetadataSaveWorkflowServicesLayer(
	liveMetadataSaveWorkflowServices,
);
