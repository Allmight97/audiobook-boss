/**
 * Solid 2 File List spike public strip: session/view reads and sync actions.
 * Solid TSX, compiler, and tauriClient stay private inside solid2/.
 */
import { resetFileListSession } from './session';
import { resetCoverThumbnailRuntime } from './thumbnails';

export { FileListIsland } from './FileListIsland';
export {
	clearFiles,
	clearSelection,
	loadFileList,
	removeFile,
	reorderFiles,
	restoreImportOrder,
	selectAll,
	selectFile,
	setOrderLocked,
	toggleSort,
} from './session';
export { readFileListView } from './view';
export { getFileListCoverThumbnailState, setCoverThumbnailLoader } from './thumbnails';

export function resetFileList(): void {
	resetFileListSession();
	resetCoverThumbnailRuntime();
}
