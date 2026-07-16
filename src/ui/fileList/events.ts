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

export type FileListDragHandlers = {
	onDragStart: (index: number, event: DragEvent) => void;
	onDragOver: (index: number, event: DragEvent) => void;
	onDrop: (index: number, event: DragEvent) => void;
	onDragEnd: () => void;
	consumePostDragClick: () => boolean;
};

export function createFileListDragHandlers(
	setDragState: (state: FileListDragState) => void,
): FileListDragHandlers {
	let draggedIndex: number | null = null;
	let suppressPostDragClick = false;

	function resetDragState(): void {
		draggedIndex = null;
		setDragState({ draggedIndex: null, hoveredIndex: null });
	}

	return {
		onDragStart(index: number, event: DragEvent) {
			if (get(metadataSaveInProgress) || isOrderLocked()) return;
			if (!event.dataTransfer || !hasValidIndex(index)) return;

			const item = event.currentTarget as HTMLElement | null;
			if (!item) return;

			draggedIndex = index;
			event.dataTransfer.effectAllowed = 'move';
			event.dataTransfer.setData('text/plain', index.toString());
			setDragState({ draggedIndex: index, hoveredIndex: null });
		},
		onDragOver(index: number, event: DragEvent) {
			if (get(metadataSaveInProgress)) return;
			if (isOrderLocked()) return;
			event.preventDefault();
			if (!event.dataTransfer) return;

			event.dataTransfer.dropEffect = 'move';
			if (!hasValidIndex(index) || draggedIndex === null) return;

			setDragState({
				draggedIndex,
				hoveredIndex: draggedIndex === index ? null : index,
			});
		},
		onDrop(index: number, event: DragEvent) {
			if (get(metadataSaveInProgress)) return;
			if (isOrderLocked()) return;
			event.preventDefault();
			event.stopPropagation();

			if (draggedIndex === null || draggedIndex === index) {
				suppressPostDragClick = draggedIndex !== null;
				resetDragState();
				return;
			}
			if (!hasValidIndex(index)) {
				suppressPostDragClick = true;
				resetDragState();
				return;
			}

			suppressPostDragClick = true;
			reorderFiles(draggedIndex, index);
			resetDragState();
		},
		onDragEnd() {
			suppressPostDragClick ||= draggedIndex !== null;
			resetDragState();
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
