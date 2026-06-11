/**
 * File list module — pre-processing workbench file list state, actions, and rendering.
 */
export {
	appendFileList,
	clearAllFiles,
	clearSelectionAction,
	displayFileList,
	moveFileDown,
	moveFileUp,
	recalculateTotals,
	removeFile,
	reorderFiles,
	selectAll,
	selectFile,
	setFileOrderLocked,
	toggleFileSort,
} from './actions';

export {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	getSelectedFiles,
	getSortAscending,
	isOrderLocked,
	onOrderLockChange,
	setCurrentFileList,
	setOrderLocked,
	setSelectedFileIndices,
	setSelectedIndex,
	setSortAscending,
} from './state.svelte';

export {
	persistPendingMetadataDraftsForCurrentSelection,
	persistSingleSelectionMetadata,
	preserveMetadataDraftsBeforeSelectionChange,
	stageMetadataToSelection,
} from './metadataStaging';

export {
	autoUpdateCoverArtFromFirstValidFile,
	clearSelectionPanels,
	ensureMetadataForFiles,
	refreshSelectionPresentation,
	showMultiSelection,
	showSingleSelection,
	updateFileProperties,
} from './metadataPanel';

export type { FileListAppendOutcome, FileListAppendResult } from './appendResult';
export {
	buildFileListAppendResult,
	buildFileListInfoFromFiles,
	buildSelectedDecoderByPath,
	collectUniqueFiles,
	normalizeFileListInfo,
} from './appendResult';

export { readCombinedSizeText } from './viewState.svelte';

export { default as FileListIsland } from './FileListIsland.svelte';
