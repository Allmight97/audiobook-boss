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
};

export type FileListRowHitTest = (clientX: number, clientY: number) => number | null;

export type FileListPointerReorderHandlers = {
	onGripPointerDown: (index: number, event: PointerEvent) => void;
	consumePostDragClick: () => boolean;
};

/** Pixels of pointer travel before a grip press becomes a drag. */
const REORDER_DRAG_THRESHOLD_PX = 4;

/**
 * Default hit test: the row index under the pointer. jsdom has no layout, so
 * tests inject their own hit test instead of stubbing elementFromPoint.
 */
export function fileListRowIndexFromPoint(clientX: number, clientY: number): number | null {
	if (typeof document.elementFromPoint !== 'function') return null;
	const row = document
		.elementFromPoint(clientX, clientY)
		?.closest<HTMLElement>('tr[data-file-index]');
	const index = row ? Number.parseInt(row.dataset.fileIndex ?? '', 10) : Number.NaN;
	return Number.isInteger(index) ? index : null;
}

/**
 * Pointer-based reorder. Internal reorder must never enter the OS drag layer:
 * HTML5 drags start a native drag session that Tauri/wry intercepts as
 * file-import ingress (wry emits drag-enter for any NSDraggingInfo).
 */
export function createFileListPointerReorder(
	setDragState: (state: FileListDragState) => void,
	hitTest: FileListRowHitTest = fileListRowIndexFromPoint,
): FileListPointerReorderHandlers {
	let pressedIndex: number | null = null;
	let pressedPointerId: number | null = null;
	let startX = 0;
	let startY = 0;
	let dragEngaged = false;
	let draggedIndex: number | null = null;
	let hoveredIndex: number | null = null;
	let suppressPostDragClick = false;

	function detachWindowListeners(): void {
		window.removeEventListener('pointermove', handlePointerMove);
		window.removeEventListener('pointerup', handlePointerUp);
		window.removeEventListener('pointercancel', handlePointerCancel);
	}

	function resetDragState(): void {
		pressedIndex = null;
		pressedPointerId = null;
		dragEngaged = false;
		draggedIndex = null;
		hoveredIndex = null;
		detachWindowListeners();
		setDragState({ draggedIndex: null, hoveredIndex: null });
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

		const targetIndex = hitTest(event.clientX, event.clientY);
		hoveredIndex =
			targetIndex !== null && targetIndex !== draggedIndex && hasValidIndex(targetIndex)
				? targetIndex
				: null;
		setDragState({ draggedIndex, hoveredIndex });
	}

	function handlePointerUp(event: PointerEvent): void {
		if (event.pointerId !== pressedPointerId) return;

		if (dragEngaged) {
			// Only an engaged drag suppresses the click that follows pointerup.
			suppressPostDragClick = true;
			const from = draggedIndex;
			const to = hoveredIndex;
			if (
				from !== null &&
				to !== null &&
				from !== to &&
				!get(metadataSaveInProgress) &&
				!isOrderLocked()
			) {
				reorderFiles(from, to);
			}
		}
		resetDragState();
	}

	function handlePointerCancel(event: PointerEvent): void {
		if (event.pointerId !== pressedPointerId) return;
		suppressPostDragClick ||= dragEngaged;
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
			suppressPostDragClick = false;
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
