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
	type FileListReorderCommand,
} from './keyboardNavigation';
import {
	clearSelectionAction,
	applySelectionIntent,
	moveFileDown,
	moveFileUp,
	reorderFiles,
} from './actions';

export type FileListDragState = {
	draggedIndex: number | null;
	hoveredIndex: number | null;
	hoveredEdge: 'top' | 'bottom' | null;
};

/** A hovered row plus which half of it the pointer is over. */
export type FileListRowHit = { index: number; edge: 'top' | 'bottom' };

export type FileListRowHitTest = (clientX: number, clientY: number) => FileListRowHit | null;

export type FileListPointerReorderHandlers = {
	onGripPointerDown: (index: number, event: PointerEvent) => void;
	consumePostDragClick: () => boolean;
};

/** Pixels of pointer travel before a grip press becomes a drag. */
const REORDER_DRAG_THRESHOLD_PX = 4;

/**
 * Default hit test: the row under the pointer plus which half it's in. jsdom
 * has no layout, so tests inject their own hit test instead of stubbing
 * elementFromPoint/getBoundingClientRect.
 */
export function fileListRowHitFromPoint(clientX: number, clientY: number): FileListRowHit | null {
	if (typeof document.elementFromPoint !== 'function') return null;
	const row = document
		.elementFromPoint(clientX, clientY)
		?.closest<HTMLElement>('tr[data-file-index]');
	if (!row) return null;
	const index = Number.parseInt(row.dataset.fileIndex ?? '', 10);
	if (!Number.isInteger(index)) return null;
	const rect = row.getBoundingClientRect();
	const edge = clientY < rect.top + rect.height / 2 ? 'top' : 'bottom';
	return { index, edge };
}

/**
 * Pointer-based reorder. Internal reorder must never enter the OS drag layer:
 * HTML5 drags start a native drag session that Tauri/wry intercepts as
 * file-import ingress (wry emits drag-enter for any NSDraggingInfo).
 */
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
		// The immediate post-drag click (if any) fires before the user's next
		// pointerdown, so this only ever disarms a click that never arrived.
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
			const travel = Math.hypot(event.clientX - startX, event.clientY - startY);
			if (travel < REORDER_DRAG_THRESHOLD_PX) return;
			dragEngaged = true;
			draggedIndex = pressedIndex;
		}

		const hit = hitTest(event.clientX, event.clientY);
		if (hit !== null && hit.index !== draggedIndex && hasValidIndex(hit.index)) {
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
			// Only an engaged drag suppresses the click that follows pointerup.
			armPostDragClickSuppression();
			const from = draggedIndex;
			if (
				from !== null &&
				hoveredIndex !== null &&
				hoveredEdge !== null &&
				!get(metadataSaveInProgress) &&
				!isOrderLocked()
			) {
				// The indicator promises "insert above" (top) or "insert below"
				// (bottom) of the hovered row; compensate for reorderFiles'
				// remove-then-insert splice so the drop lands where it's shown.
				const insertIndex = hoveredEdge === 'top' ? hoveredIndex : hoveredIndex + 1;
				const to = from < insertIndex ? insertIndex - 1 : insertIndex;
				if (to !== from) reorderFiles(from, to);
			}
		}
		resetDragState();
	}

	function handlePointerCancel(event: PointerEvent): void {
		if (event.pointerId !== pressedPointerId) return;
		// A cancelled pointer emits no click, so arming suppression here would
		// eat the user's NEXT intentional click instead.
		resetDragState();
	}

	return {
		onGripPointerDown(index: number, event: PointerEvent) {
			if (event.button !== 0) return;
			if (get(metadataSaveInProgress) || isOrderLocked()) return;
			if (!hasValidIndex(index)) return;

			pressedIndex = index;
			pressedPointerId = event.pointerId;
			startX = event.clientX;
			startY = event.clientY;
			event.preventDefault();

			const grip = event.currentTarget;
			if (grip instanceof Element && typeof grip.setPointerCapture === 'function') {
				try {
					grip.setPointerCapture(event.pointerId);
					// Losing capture mid-drag (e.g. the grip unmounting) abandons the
					// drag like a cancel. On a normal pointerup this fires after state
					// is already reset, so the pointer-id guard makes it a no-op.
					// Tracked so resetDragState can remove it if it never fires,
					// avoiding a stale listener surviving pointer-id reuse.
					lostPointerCaptureListener = (lostEvent) =>
						handlePointerCancel(lostEvent as PointerEvent);
					pressedGrip = grip;
					grip.addEventListener('lostpointercapture', lostPointerCaptureListener, {
						once: true,
					});
				} catch {
					// Capture is best-effort; window listeners carry the drag regardless.
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

function handleKeyboardReorder(event: KeyboardEvent, command: FileListReorderCommand): boolean {
	if (isOrderLocked()) return false;

	const index = getSelectedFileIndex();
	if (!hasValidIndex(index)) return false;

	event.preventDefault();
	if (command === 'moveUp') {
		if (index > 0) moveFileUp(index);
		return true;
	}

	const fileList = getCurrentFileList();
	if (fileList && index < fileList.files.length - 1) {
		moveFileDown(index);
	}
	return true;
}

function isCurrentSingleSelection(index: number): boolean {
	const selectedIndices = getSelectedFileIndices();
	return selectedIndices.size === 1 && selectedIndices.has(index);
}

function handleKeyboardNavigation(event: KeyboardEvent): boolean {
	const command = fileListNavigationCommandFromKey(event);
	if (!command) return false;

	if (command === 'moveUp' || command === 'moveDown') {
		return handleKeyboardReorder(event, command);
	}

	const fileList = getCurrentFileList();
	if (!fileList) return false;

	const targetIndex = resolveFileListNavigationTarget({
		command,
		fileCount: fileList.files.length,
		selectedIndex: getSelectedFileIndex(),
	});
	if (targetIndex === null) return false;

	event.preventDefault();
	if (isCurrentSingleSelection(targetIndex)) {
		return true;
	}

	void applySelectionIntent({ type: 'selectOnly', index: targetIndex });
	return true;
}

export function onFileListKeyDown(e: KeyboardEvent): void {
	if (get(metadataSaveInProgress)) return;
	if (!getCurrentFileList()) return;
	if (isTextInputTarget(e.target)) return;

	if (handleKeyboardNavigation(e)) {
		return;
	}

	const key = e.key.toLowerCase();
	if ((e.metaKey || e.ctrlKey) && key === 'a') {
		e.preventDefault();
		void applySelectionIntent({ type: 'selectAll' });
		return;
	}

	if (key === 'escape') {
		e.preventDefault();
		void clearSelectionAction();
	}
}

function isTextInputTarget(target: EventTarget | null): boolean {
	if (!target || !(target instanceof HTMLElement)) return false;
	const tagName = target.tagName.toLowerCase();
	return tagName === 'input' || tagName === 'textarea';
}
