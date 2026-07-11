import {
	getCurrentFileList,
	getSelectedFileIndex,
	setSelectedIndex,
	getSelectedFileIndices,
	addToSelectedIndices,
	clearSelectedIndices,
	removeFromSelectedIndices,
	setSelectedFileIndices,
} from './state.svelte';

export type SelectionIntent =
	| { type: 'selectOnly'; index: number }
	| { type: 'toggle'; index: number }
	| { type: 'selectAll' }
	| { type: 'clear' };

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

export function applySelectionIntent(intent: SelectionIntent): SelectionResult {
	const fileList = getCurrentFileList();
	if (!fileList) return { changed: false };

	const totalFiles = fileList.files.length;

	switch (intent.type) {
		case 'selectOnly': {
			if (intent.index < 0 || intent.index >= totalFiles) return { changed: false };
			const selected = getSelectedFileIndices();
			if (selected.size === 1 && selected.has(intent.index) && getSelectedFileIndex() === intent.index) {
				return { changed: false };
			}
			setSelectedFileIndices([intent.index]);
			setSelectedIndex(intent.index);
			return { changed: true };
		}
		case 'toggle': {
			if (intent.index < 0 || intent.index >= totalFiles) return { changed: false };
			const selected = getSelectedFileIndices();
			if (selected.has(intent.index)) {
				removeFromSelectedIndices(intent.index);
				ensureAnchor();
			} else {
				addToSelectedIndices(intent.index);
				setSelectedIndex(intent.index);
			}
			return { changed: true };
		}
		case 'selectAll': {
			if (totalFiles === 0) return { changed: false };
			const selected = getSelectedFileIndices();
			if (selected.size === totalFiles) return { changed: false };
			setSelectedFileIndices(Array.from({ length: totalFiles }, (_, index) => index));
			if (getSelectedFileIndex() < 0 || !getSelectedFileIndices().has(getSelectedFileIndex())) {
				setSelectedIndex(0);
			}
			return { changed: true };
		}
		case 'clear': {
			if (getSelectedFileIndices().size === 0 && getSelectedFileIndex() === -1) {
				return { changed: false };
			}
			clearSelectedIndices();
			setSelectedIndex(-1);
			return { changed: true };
		}
	}
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
