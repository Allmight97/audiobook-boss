import { beforeEach, describe, expect, it } from 'vitest';

import * as fileList from '..';
import { setCurrentFileList } from '../state.svelte';

const EXPECTED_FILE_LIST_EXPORTS = [
	'FileListIsland',
	'appendFileList',
	'applySelectionIntent',
	'getCurrentFileList',
	'getSelectedFileIndex',
	'getSelectedFileIndices',
	'getSelectedFiles',
	'isOrderLocked',
	'onOrderLockChange',
	'openMetadataSurfaceForCurrentSelection',
	'persistPendingMetadataDraftsForCurrentSelection',
	'readActiveFileChapters',
	'readCombinedDurationText',
	'readCombinedSizeText',
	'readFileListCount',
	'readInspectorFacts',
	'removeSelectedFiles',
	'requestMetadataSurfaceDismissal',
	'selectFile',
	'setFileOrderLocked',
	'setMetadataSurfacePresentation',
	'stageMetadataToSelection',
] as const;

describe('File List Runtime public API contract', () => {
	beforeEach(() => {
		setCurrentFileList(null);
	});

	it('pins the file list public export strip', () => {
		expect(Object.keys(fileList).sort()).toEqual([...EXPECTED_FILE_LIST_EXPORTS].sort());
	});

	it('reads combined size through the public accessor', () => {
		expect(fileList.readCombinedSizeText()).toBe('--- MB');
		expect(fileList.readCombinedDurationText()).toBe('---');
		expect(fileList.readFileListCount()).toBe(0);
	});
});
