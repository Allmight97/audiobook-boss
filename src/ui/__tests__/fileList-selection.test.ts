import { describe, it, expect, beforeEach } from 'vitest';
import {
	handleSelection,
	selectAllFiles,
	clearSelection,
	reindexSelectionAfterRemoval,
	reindexSelectionAfterMove,
	swapSelectionIndices,
} from '../fileList/selection';
import {
	getCurrentFileList,
	getSelectedFileIndex,
	setCurrentFileList,
	setSelectedIndex,
	setSelectedFileIndices,
	clearSelectedIndices,
	getSelectedFileIndices,
} from '../fileList/state.svelte';
import type { AudioFile, FileListInfo } from '../../types/audio';

const makeFileList = (count: number): FileListInfo => {
	const files: AudioFile[] = Array.from({ length: count }, (_, index) => ({
		path: `/tmp/file-${index}.m4b`,
		isValid: true,
	}));

	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: 0,
		totalSize: 0,
		validCount: count,
		invalidCount: 0,
	};
};

const selectedIndices = (): number[] => Array.from(getSelectedFileIndices()).sort((a, b) => a - b);

describe('file list selection', () => {
	beforeEach(() => {
		setCurrentFileList(makeFileList(5));
		clearSelectedIndices();
		setSelectedIndex(-1);
	});

	it('selects a single item and updates anchor', () => {
		const result = handleSelection(2, { multi: false, range: false });

		expect(result.changed).toBe(true);
		expect(selectedIndices()).toEqual([2]);
		expect(getSelectedFileIndex()).toBe(2);
	});

	it('toggles multi-select and maintains anchor', () => {
		handleSelection(1, { multi: false, range: false });
		handleSelection(3, { multi: true, range: false });

		expect(selectedIndices()).toEqual([1, 3]);
		expect(getSelectedFileIndex()).toBe(3);

		handleSelection(3, { multi: true, range: false });

		expect(selectedIndices()).toEqual([1]);
		expect(getSelectedFileIndex()).toBe(1);
	});

	it('selects a range from the anchor', () => {
		handleSelection(1, { multi: false, range: false });
		handleSelection(3, { multi: false, range: true });

		expect(selectedIndices()).toEqual([1, 2, 3]);
		expect(getSelectedFileIndex()).toBe(3);
	});

	it('selects all files', () => {
		const changed = selectAllFiles();

		expect(changed).toBe(true);
		expect(selectedIndices()).toEqual([0, 1, 2, 3, 4]);
		expect(getSelectedFileIndex()).toBe(0);
	});

	it('clears selection', () => {
		handleSelection(2, { multi: false, range: false });

		const changed = clearSelection();

		expect(changed).toBe(true);
		expect(selectedIndices()).toEqual([]);
		expect(getSelectedFileIndex()).toBe(-1);
	});

	it('reindexes selection after removal', () => {
		setSelectedFileIndices([1, 3, 4]);
		setSelectedIndex(4);

		reindexSelectionAfterRemoval(2);

		expect(selectedIndices()).toEqual([1, 2, 3]);
		expect(getSelectedFileIndex()).toBe(3);
	});

	it('reindexes selection after move', () => {
		setSelectedFileIndices([1, 4]);
		setSelectedIndex(4);

		reindexSelectionAfterMove(1, 3);

		expect(selectedIndices()).toEqual([3, 4]);
		expect(getSelectedFileIndex()).toBe(4);
	});

	it('swaps selection indices and anchor', () => {
		setSelectedFileIndices([1, 3]);
		setSelectedIndex(1);

		swapSelectionIndices(1, 2);

		expect(selectedIndices()).toEqual([2, 3]);
		expect(getSelectedFileIndex()).toBe(2);
	});

	it('ignores out-of-bounds selections', () => {
		const result = handleSelection(9, { multi: false, range: false });

		expect(result.changed).toBe(false);
		expect(getCurrentFileList()?.files.length).toBe(5);
		expect(selectedIndices()).toEqual([]);
	});

	it('resets selection state when a fresh file list is loaded', () => {
		setSelectedFileIndices([1, 3]);
		setSelectedIndex(3);

		setCurrentFileList(makeFileList(2));

		expect(getSelectedFileIndex()).toBe(-1);
		expect(selectedIndices()).toEqual([]);
	});
});
