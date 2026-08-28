/**
 * File list module — leftover Svelte workbench adapters used by unmigrated owners.
 */
export { appendFileList, selectFile, setFileOrderLocked } from './actions';

export {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	getSelectedFiles,
	isOrderLocked,
	onOrderLockChange,
} from './state.svelte';

export {
	persistPendingMetadataDraftsForCurrentSelection,
	stageMetadataToSelection,
} from './metadataStaging';

export { readCombinedSizeText } from './viewState.svelte';
