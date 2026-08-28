/**
 * File list module — pre-processing workbench file list state, actions, and rendering.
 */
export { default as FileListIsland } from './FileListIsland.svelte';

export { appendFileList, selectFile, setFileOrderLocked } from './actions';

export {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	getSelectedFiles,
	isOrderLocked,
	onOrderLockChange,
} from './state';

export {
	persistPendingMetadataDraftsForCurrentSelection,
	stageMetadataToSelection,
} from './metadataStaging';

export { readCombinedSizeText } from './viewState';
