import {
	getCurrentFileList,
	getSelectedFileIndex,
	setSelectedIndex,
	getSelectedFileIndices,
	addToSelectedIndices,
	clearSelectedIndices,
	removeFromSelectedIndices,
	setSelectedFileIndices,
} from './state';

type SelectionModifiers = { multi: boolean; range: boolean };

type SelectionResult = {
	changed: boolean;
};

function getSortedSelectedIndices(): number[] {
	return Array.from(getSelectedFileIndices()).sort((a, b) => a - b);
}

function ensureAnchor(): void {
	const selected = getSelectedFileIndices();
	if (selected.size === 0) {
		setSelectedIndex(-1);
		return;
	}

	const selectedFileIndex = getSelectedFileIndex();
	if (selectedFileIndex >= 0 && selected.has(selectedFileIndex)) return;

	const sorted = getSortedSelectedIndices();
	const newAnchor = sorted[sorted.length - 1];
	setSelectedIndex(newAnchor);
}

export function handleSelection(index: number, modifiers: SelectionModifiers): SelectionResult {
	const fileList = getCurrentFileList();
	if (!fileList) return { changed: false };

	const totalFiles = fileList.files.length;
	if (index < 0 || index >= totalFiles) return { changed: false };

	const { multi, range } = modifiers;
	const selectedFileIndex = getSelectedFileIndex();

	if (range && selectedFileIndex !== -1) {
		const start = Math.min(selectedFileIndex, index);
		const end = Math.max(selectedFileIndex, index);

		clearSelectedIndices();
		for (let i = start; i <= end; i += 1) {
			addToSelectedIndices(i);
		}
		setSelectedIndex(index);
		return { changed: true };
	}

	if (multi) {
		const selected = getSelectedFileIndices();
		if (selected.has(index)) {
			removeFromSelectedIndices(index);
		} else {
			addToSelectedIndices(index);
			setSelectedIndex(index);
		}

		ensureAnchor();
		return { changed: true };
	}

	clearSelectedIndices();
	addToSelectedIndices(index);
	setSelectedIndex(index);
	return { changed: true };
}

export function selectAllFiles(): boolean {
	const fileList = getCurrentFileList();
	if (!fileList) return false;

	clearSelectedIndices();
	const count = fileList.files.length;
	for (let i = 0; i < count; i += 1) {
		addToSelectedIndices(i);
	}
	setSelectedIndex(count > 0 ? 0 : -1);
	return count > 0;
}

export function clearSelection(): boolean {
	const selectedFileIndex = getSelectedFileIndex();
	if (getSelectedFileIndices().size === 0 && selectedFileIndex === -1) {
		return false;
	}
	clearSelectedIndices();
	setSelectedIndex(-1);
	return true;
}

export function reindexSelectionAfterRemoval(removedIndex: number): void {
	const updated = new Set<number>();
	getSelectedFileIndices().forEach((index) => {
		if (index === removedIndex) return;
		updated.add(index > removedIndex ? index - 1 : index);
	});

	setSelectedFileIndices(updated);
	ensureAnchor();
}

function mapIndexForMove(index: number, fromIndex: number, toIndex: number): number {
	if (index === fromIndex) return toIndex;
	if (fromIndex < toIndex && index > fromIndex && index <= toIndex) return index - 1;
	if (fromIndex > toIndex && index >= toIndex && index < fromIndex) return index + 1;
	return index;
}

export function reindexSelectionAfterMove(fromIndex: number, toIndex: number): void {
	const updated = new Set<number>();
	getSelectedFileIndices().forEach((index) => {
		updated.add(mapIndexForMove(index, fromIndex, toIndex));
	});

	setSelectedFileIndices(updated);

	const selectedFileIndex = getSelectedFileIndex();
	if (selectedFileIndex !== -1) {
		setSelectedIndex(mapIndexForMove(selectedFileIndex, fromIndex, toIndex));
	}

	ensureAnchor();
}

export function swapSelectionIndices(indexA: number, indexB: number): void {
	const updated = new Set<number>();
	getSelectedFileIndices().forEach((index) => {
		if (index === indexA) {
			updated.add(indexB);
		} else if (index === indexB) {
			updated.add(indexA);
		} else {
			updated.add(index);
		}
	});

	setSelectedFileIndices(updated);

	const selectedFileIndex = getSelectedFileIndex();
	if (selectedFileIndex === indexA) {
		setSelectedIndex(indexB);
	} else if (selectedFileIndex === indexB) {
		setSelectedIndex(indexA);
	}

	ensureAnchor();
}
