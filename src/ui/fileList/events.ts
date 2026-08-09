import { get } from 'svelte/store';

import {
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	isOrderLocked,
} from './state.svelte';
import { metadataSaveInProgress } from '../metadataSession';
import {
	fileListNavigationCommandFromKey,
	resolveFileListNavigationTarget,
} from './keyboardNavigation';
import {
	clearSelectionAction,
	moveFileDown,
	moveFileUp,
	reorderFiles,
	removeFile,
	selectAll,
	selectFile,
} from './actions';

export type FileListDragState = {
	draggedIndex: number | null;
	hoveredIndex: number | null;
	hoveredEdge: 'top' | 'bottom' | null;
};

export type FileListRowHit = { index: number; edge: 'top' | 'bottom' };
export type FileListRowHitTest = (clientX: number, clientY: number) => FileListRowHit | null;
export type FileListPointerReorderHandlers = {
	onGripPointerDown: (index: number, event: PointerEvent) => void;
	consumePostDragClick: () => boolean;
};

const REORDER_DRAG_THRESHOLD_PX = 4;

export function fileListRowHitFromPoint(clientX: number, clientY: number): FileListRowHit | null {
	if (typeof document.elementFromPoint !== 'function') return null;
	const row = document
		.elementFromPoint(clientX, clientY)
		?.closest<HTMLElement>('[data-file-index]');
	if (!row) return null;
	const index = Number.parseInt(row.dataset.fileIndex ?? '', 10);
	if (!Number.isInteger(index)) return null;
	const rect = row.getBoundingClientRect();
	return { index, edge: clientY < rect.top + rect.height / 2 ? 'top' : 'bottom' };
}

/** Internal queue reorder uses pointer events so it cannot enter Tauri's
 * native file-drop ingress path. */
export function createFileListPointerReorder(
	setDragState: (state: FileListDragState) => void,
	hitTest: FileListRowHitTest = fileListRowHitFromPoint,
): FileListPointerReorderHandlers {
	let pressedIndex: number | null = null;
	let pressedPointerId: number | null = null;
	let startX = 0;
	let startY = 0;
	let dragEngaged = false;
	let draggedIndex: number | null = null;
	let hoveredIndex: number | null = null;
	let hoveredEdge: 'top' | 'bottom' | null = null;
	let suppressPostDragClick = false;
	let pressedGrip: Element | null = null;
	let lostPointerCaptureListener: ((event: Event) => void) | null = null;

	function detachWindowListeners(): void {
		window.removeEventListener('pointermove', handlePointerMove);
		window.removeEventListener('pointerup', handlePointerUp);
		window.removeEventListener('pointercancel', handlePointerCancel);
	}
	function detachLostPointerCaptureListener(): void {
		if (pressedGrip && lostPointerCaptureListener) {
			pressedGrip.removeEventListener('lostpointercapture', lostPointerCaptureListener);
		}
		pressedGrip = null;
		lostPointerCaptureListener = null;
	}
	function disarmPostDragClickSuppression(): void {
		suppressPostDragClick = false;
		window.removeEventListener('pointerdown', disarmPostDragClickSuppression, true);
	}
	function armPostDragClickSuppression(): void {
		suppressPostDragClick = true;
		window.addEventListener('pointerdown', disarmPostDragClickSuppression, true);
	}
	function resetDragState(): void {
		pressedIndex = null;
		pressedPointerId = null;
		dragEngaged = false;
		draggedIndex = null;
		hoveredIndex = null;
		hoveredEdge = null;
		detachWindowListeners();
		detachLostPointerCaptureListener();
		setDragState({ draggedIndex: null, hoveredIndex: null, hoveredEdge: null });
	}
	function handlePointerMove(event: PointerEvent): void {
		if (event.pointerId !== pressedPointerId || pressedIndex === null) return;
		if (get(metadataSaveInProgress) || isOrderLocked()) {
			resetDragState();
			return;
		}
		if (!dragEngaged) {
			if (Math.hypot(event.clientX - startX, event.clientY - startY) < REORDER_DRAG_THRESHOLD_PX)
				return;
			dragEngaged = true;
			draggedIndex = pressedIndex;
		}
		const hit = hitTest(event.clientX, event.clientY);
		if (hit && hit.index !== draggedIndex && hasValidIndex(hit.index)) {
			hoveredIndex = hit.index;
			hoveredEdge = hit.edge;
		} else {
			hoveredIndex = null;
			hoveredEdge = null;
		}
		setDragState({ draggedIndex, hoveredIndex, hoveredEdge });
	}
	function handlePointerUp(event: PointerEvent): void {
		if (event.pointerId !== pressedPointerId) return;
		if (dragEngaged) {
			armPostDragClickSuppression();
			const from = draggedIndex;
			if (
				from !== null &&
				hoveredIndex !== null &&
				hoveredEdge !== null &&
				!get(metadataSaveInProgress) &&
				!isOrderLocked()
			) {
				const insertIndex = hoveredEdge === 'top' ? hoveredIndex : hoveredIndex + 1;
				const to = from < insertIndex ? insertIndex - 1 : insertIndex;
				if (to !== from) reorderFiles(from, to);
			}
		}
		resetDragState();
	}
	function handlePointerCancel(event: PointerEvent): void {
		if (event.pointerId !== pressedPointerId) return;
		resetDragState();
	}
	return {
		onGripPointerDown(index, event) {
			if (
				event.button !== 0 ||
				get(metadataSaveInProgress) ||
				isOrderLocked() ||
				!hasValidIndex(index)
			)
				return;
			pressedIndex = index;
			pressedPointerId = event.pointerId;
			startX = event.clientX;
			startY = event.clientY;
			event.preventDefault();
			const grip = event.currentTarget;
			if (grip instanceof Element && typeof grip.setPointerCapture === 'function') {
				try {
					grip.setPointerCapture(event.pointerId);
					lostPointerCaptureListener = (lostEvent) =>
						handlePointerCancel(lostEvent as PointerEvent);
					pressedGrip = grip;
					grip.addEventListener('lostpointercapture', lostPointerCaptureListener, { once: true });
				} catch {
					// Window listeners still carry the gesture when capture is unavailable.
				}
			}
			window.addEventListener('pointermove', handlePointerMove);
			window.addEventListener('pointerup', handlePointerUp);
			window.addEventListener('pointercancel', handlePointerCancel);
		},
		consumePostDragClick() {
			const shouldSuppress = suppressPostDragClick;
			if (shouldSuppress) disarmPostDragClickSuppression();
			return shouldSuppress;
		},
	};
}

function hasValidIndex(index: number): boolean {
	const fileList = getCurrentFileList();
	return Boolean(fileList && index >= 0 && index < fileList.files.length);
}

export function onFileListClick(index: number, event: MouseEvent): void {
	if (get(metadataSaveInProgress) || !hasValidIndex(index)) return;
	if (event.shiftKey && typeof window !== 'undefined') window.getSelection()?.removeAllRanges();
	void selectFile(index, { multi: event.ctrlKey || event.metaKey, range: event.shiftKey });
}

export function onFileListMoveUp(index: number, event: MouseEvent): void {
	if (get(metadataSaveInProgress) || isOrderLocked() || !hasValidIndex(index)) return;
	event.stopPropagation();
	event.preventDefault();
	moveFileUp(index);
}
export function onFileListMoveDown(index: number, event: MouseEvent): void {
	if (get(metadataSaveInProgress) || isOrderLocked() || !hasValidIndex(index)) return;
	event.stopPropagation();
	event.preventDefault();
	moveFileDown(index);
}
export function onFileListRemove(index: number, event: MouseEvent): void {
	if (get(metadataSaveInProgress) || isOrderLocked() || !hasValidIndex(index)) return;
	event.stopPropagation();
	event.preventDefault();
	void removeFile(index);
}

function handleKeyboardNavigation(event: KeyboardEvent): boolean {
	const command = fileListNavigationCommandFromKey(event);
	if (!command) return false;
	const fileList = getCurrentFileList();
	if (!fileList) return false;
	const targetIndex = resolveFileListNavigationTarget({
		command,
		fileCount: fileList.files.length,
		selectedIndex: getSelectedFileIndex(),
	});
	if (targetIndex === null) return false;
	event.preventDefault();
	const selected = getSelectedFileIndices();
	if (selected.size === 1 && selected.has(targetIndex)) return true;
	void selectFile(targetIndex, { multi: false, range: false });
	return true;
}

export function onFileListKeyDown(event: KeyboardEvent): void {
	if (get(metadataSaveInProgress) || !getCurrentFileList() || isTextInputTarget(event.target))
		return;
	if (handleKeyboardNavigation(event)) return;
	const key = event.key.toLowerCase();
	if ((event.metaKey || event.ctrlKey) && key === 'a') {
		event.preventDefault();
		void selectAll();
	} else if (key === 'escape') {
		event.preventDefault();
		void clearSelectionAction();
	}
}

function isTextInputTarget(target: EventTarget | null): boolean {
	if (!target || !(target instanceof HTMLElement)) return false;
	const tagName = target.tagName.toLowerCase();
	return tagName === 'input' || tagName === 'textarea';
}
