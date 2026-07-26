import { describe, it, expect, beforeEach } from 'vitest';
import {
	applySelectionIntent,
	reindexSelectionAfterRemoval,
	reindexSelectionAfterMove,
	swapSelectionIndices,
} from '../selection';
import {
	clearSelectedIndices,
	getCurrentFileList,
	getSelectedFileIndex,
	getSelectedFileIndices,
	setCurrentFileList,
	setSelectedFileIndices,
	setSelectedIndex,
} from '../state.svelte';
import type { AudioFile, FileListInfo } from '../../../types/audio';

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

	it('selects only one item and makes it active', () => {
		const result = applySelectionIntent({ type: 'selectOnly', index: 2 });

		expect(result.changed).toBe(true);
		expect(selectedIndices()).toEqual([2]);
		expect(getSelectedFileIndex()).toBe(2);
	});

	it('repairs the active file when toggling it off', () => {
		applySelectionIntent({ type: 'selectOnly', index: 1 });
		applySelectionIntent({ type: 'toggle', index: 3 });

		expect(selectedIndices()).toEqual([1, 3]);
		expect(getSelectedFileIndex()).toBe(3);

		applySelectionIntent({ type: 'toggle', index: 3 });

		expect(selectedIndices()).toEqual([1]);
		expect(getSelectedFileIndex()).toBe(1);
	});

	it('selects all files and clears them', () => {
		const changed = applySelectionIntent({ type: 'selectAll' });

		expect(changed.changed).toBe(true);
		expect(selectedIndices()).toEqual([0, 1, 2, 3, 4]);
		expect(getSelectedFileIndex()).toBe(0);

		const cleared = applySelectionIntent({ type: 'clear' });

		expect(cleared.changed).toBe(true);
		expect(selectedIndices()).toEqual([]);
		expect(getSelectedFileIndex()).toBe(-1);
	});

	it('selects an inclusive range and makes the clicked file active', () => {
		applySelectionIntent({ type: 'selectOnly', index: 1 });

		const result = applySelectionIntent({ type: 'range', anchorIndex: 1, index: 4 });

		expect(result.changed).toBe(true);
		expect(selectedIndices()).toEqual([1, 2, 3, 4]);
		expect(getSelectedFileIndex()).toBe(4);
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
		const result = applySelectionIntent({ type: 'selectOnly', index: 9 });

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
