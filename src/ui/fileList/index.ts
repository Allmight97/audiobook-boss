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

export {
	readActiveFileChapters,
	readCombinedDurationText,
	readCombinedSizeText,
	readFileListCount,
	readFileListOrderLockVisible,
} from './viewState.svelte';
export type { ReadWorkActivityByInputId } from './viewState.svelte';
export { readInspectorFacts } from './inspectorState.svelte';
export type { InspectorFact } from './inspectorState.svelte';

export {
	appendFileList,
	applySelectionIntent,
	removeSelectedFiles,
	selectFile,
	setFileOrderLocked,
} from './actions';
export {
	openMetadataSurfaceForCurrentSelection,
	requestMetadataSurfaceDismissal,
	coordinateMetadataSurfacePresentationRefresh,
	setMetadataSurfacePresentation,
} from './metadataPanel';
export type { MetadataSurfacePresentation } from './metadataPanel';

/**
 * File list module — pre-processing workbench file list state, actions, and rendering.
 */
export { default as FileListIsland } from './FileListIsland.svelte';
