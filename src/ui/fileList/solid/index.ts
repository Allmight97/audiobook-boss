/**
 * Solid File List spike public strip: session/view/thumbnail atoms and the
 * sync actions the island writes. Solid TSX, atom-solid hooks, and Tauri
 * loaders stay private to `solid/`.
 */
import { resetFileList as resetSession } from './session';
import { resetCoverThumbnailRuntime } from './thumbnails';

export { fileListSessionAtom } from './session';
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
export { fileListViewAtom } from './view';
export { coverThumbnailAtom, setCoverThumbnailLoader } from './thumbnails';

export function resetFileList(): void {
	resetSession();
	resetCoverThumbnailRuntime();
}
