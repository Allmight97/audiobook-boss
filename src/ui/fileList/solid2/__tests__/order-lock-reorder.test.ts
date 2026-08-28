import { beforeEach, describe, expect, it } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../../types/audio';
import {
	loadFileList,
	readFileListView,
	reorderFiles,
	resetFileList,
	restoreImportOrder,
	selectFile,
	setOrderLocked,
	toggleSort,
} from '..';

function makeFile(path: string): AudioFile {
	return { path, inputId: path, isValid: true, duration: 1, size: 1, format: 'm4b' };
}

function makeFileList(...files: AudioFile[]): FileListInfo {
	return {
		files,
		validCount: files.length,
		invalidCount: 0,
		totalDuration: files.length,
		totalSize: files.length,
		selectedDecoders: files.map(() => null),
	};
}

describe('Solid 2 File List order lock and reorder', () => {
	beforeEach(() => {
		resetFileList();
		loadFileList(
			makeFileList(
				makeFile('/books/alpha.m4b'),
				makeFile('/books/beta.m4b'),
				makeFile('/books/gamma.m4b'),
			),
		);
	});

	it('hides sort/clear while order is locked', () => {
		setOrderLocked(true);
		const view = readFileListView();
		expect(view.orderLockVisible).toBe(true);
		expect(view.sortDisabled).toBe(true);
		expect(view.clearDisabled).toBe(true);
		toggleSort();
		expect(readFileListView().sortState).toBe('none');
	});

	it('keeps selected identity across reorder and restore', () => {
		selectFile(0);
		reorderFiles(0, 2);
		expect(readFileListView().files.map((file) => file.path)).toEqual([
			'/books/beta.m4b',
			'/books/gamma.m4b',
			'/books/alpha.m4b',
		]);
		expect(readFileListView().selectedFileIndex).toBe(2);
		restoreImportOrder();
		expect(readFileListView().files.map((file) => file.path)).toEqual([
			'/books/alpha.m4b',
			'/books/beta.m4b',
			'/books/gamma.m4b',
		]);
		expect(readFileListView().selectedFileIndex).toBe(0);
		expect(readFileListView().orderDiffersFromImport).toBe(false);
	});
});
