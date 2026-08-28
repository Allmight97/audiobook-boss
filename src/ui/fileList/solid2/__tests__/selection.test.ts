import { beforeEach, describe, expect, it } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../../types/audio';
import {
	clearSelection,
	loadFileList,
	readFileListView,
	resetFileList,
	selectAll,
	selectFile,
} from '..';

function makeFileList(count: number): FileListInfo {
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
}

function selectedIndices(): number[] {
	return [...readFileListView().selectedIndices].sort((a, b) => a - b);
}

describe('Solid 2 File List selection', () => {
	beforeEach(() => {
		resetFileList();
		loadFileList(makeFileList(5));
	});

	it('selects a single item and updates anchor', () => {
		expect(selectFile(2, { multi: false, range: false })).toBe(true);
		expect(selectedIndices()).toEqual([2]);
		expect(readFileListView().selectedFileIndex).toBe(2);
	});

	it('toggles multi-select and maintains anchor', () => {
		selectFile(1, { multi: false, range: false });
		selectFile(3, { multi: true, range: false });
		expect(selectedIndices()).toEqual([1, 3]);
		expect(readFileListView().selectedFileIndex).toBe(3);

		selectFile(3, { multi: true, range: false });
		expect(selectedIndices()).toEqual([1]);
		expect(readFileListView().selectedFileIndex).toBe(1);
	});

	it('selects a range from the anchor', () => {
		selectFile(1, { multi: false, range: false });
		selectFile(3, { multi: false, range: true });
		expect(selectedIndices()).toEqual([1, 2, 3]);
		expect(readFileListView().selectedFileIndex).toBe(3);
	});

	it('selects all files', () => {
		expect(selectAll()).toBe(true);
		expect(selectedIndices()).toEqual([0, 1, 2, 3, 4]);
		expect(readFileListView().selectedFileIndex).toBe(0);
	});

	it('clears selection', () => {
		selectFile(2, { multi: false, range: false });
		expect(clearSelection()).toBe(true);
		expect(selectedIndices()).toEqual([]);
		expect(readFileListView().selectedFileIndex).toBe(-1);
	});

	it('ignores out-of-bounds selections', () => {
		expect(selectFile(9, { multi: false, range: false })).toBe(false);
		expect(readFileListView().files.length).toBe(5);
		expect(selectedIndices()).toEqual([]);
	});

	it('resets selection state when a fresh file list is loaded', () => {
		selectFile(1, { multi: true, range: false });
		loadFileList(makeFileList(2));
		expect(readFileListView().selectedFileIndex).toBe(-1);
		expect(selectedIndices()).toEqual([]);
	});
});
