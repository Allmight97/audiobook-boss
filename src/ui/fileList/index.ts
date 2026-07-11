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

export { readCombinedDurationText, readCombinedSizeText, readFileListCount } from './viewState.svelte';
export type { ReadWorkActivityByInputId } from './viewState.svelte';

export {
	appendFileList,
	applySelectionIntent,
	removeSelectedFiles,
	selectFile,
	setFileOrderLocked,
} from './actions';

/**
 * File list module — pre-processing workbench file list state, actions, and rendering.
 */
export { default as FileListIsland } from './FileListIsland.svelte';
