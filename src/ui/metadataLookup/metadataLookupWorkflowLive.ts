import { tauriClient } from '../../lib/tauri/client';
import { clearCoverArt, refreshCoverArtDisplay, setCoverArt, setCustomCoverArt } from '../coverArt';
import { selectFile } from '../fileList/actions';
import { getCurrentFileList, getSelectedFileIndices } from '../fileList/state.svelte';
import { applyMetadataToForm, readMetadataForm } from '../metadataForm';
import { getMetadataForFile, setMetadataForFile } from '../metadataState';
import { updateEstimatedSize, updateOutputPath } from '../outputPanel';
import { updateTagPreview } from '../tagPreview';
import {
	clearMetadataLookupQueue,
	metadataLookupQueueState,
	metadataLookupState,
	setMetadataLookupQueue,
	setMetadataLookupQueueIndex,
} from './state.svelte';
import {
	makeMetadataLookupWorkflowServicesLayer,
	type MetadataLookupWorkflowServices,
} from './metadataLookupWorkflowServices';

const liveMetadataLookupWorkflowServices: MetadataLookupWorkflowServices = {
	getLookupState: () => metadataLookupState,
	getQueueState: () => metadataLookupQueueState,
	setMetadataLookupQueue,
	clearMetadataLookupQueue,
	setMetadataLookupQueueIndex,
	getSelectedFileIndices,
	getCurrentFileList,
	getMetadataForFile,
	setMetadataForFile,
	selectFile,
	applyMetadataToForm,
	readMetadataForm,
	updateOutputPath,
	updateEstimatedSize,
	updateTagPreview,
	clearCoverArt,
	setCoverArt,
	setCustomCoverArt,
	refreshCoverArtDisplay,
	searchOnlineMetadata: tauriClient.searchOnlineMetadata,
	loadCoverArtFromUrl: tauriClient.loadCoverArtFromUrl,
	focusElementById: (id) => {
		const element = document.getElementById(id);
		if (element instanceof HTMLElement) {
			element.focus();
		}
	},
	queueMicrotask,
	console,
};

export const MetadataLookupWorkflowLive = makeMetadataLookupWorkflowServicesLayer(
	liveMetadataLookupWorkflowServices,
);
