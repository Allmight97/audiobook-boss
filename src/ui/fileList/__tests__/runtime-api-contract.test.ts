import { beforeEach, describe, expect, it } from 'vitest';

import * as fileList from '..';

const EXPECTED_FILE_LIST_EXPORTS = [
	'FileListIsland',
	'appendFileList',
	'autoUpdateCoverArtFromFirstValidFile',
	'buildFileListAppendResult',
	'buildFileListInfoFromFiles',
	'buildSelectedDecoderByPath',
	'clearAllFiles',
	'clearSelectionAction',
	'clearSelectionPanels',
	'collectUniqueFiles',
	'displayFileList',
	'ensureMetadataForFiles',
	'getCurrentFileList',
	'getSelectedFileIndex',
	'getSelectedFileIndices',
	'getSelectedFiles',
	'getSortAscending',
	'isOrderLocked',
	'moveFileDown',
	'moveFileUp',
	'normalizeFileListInfo',
	'onOrderLockChange',
	'persistPendingMetadataDraftsForCurrentSelection',
	'persistSingleSelectionMetadata',
	'preserveMetadataDraftsBeforeSelectionChange',
	'readCombinedSizeText',
	'recalculateTotals',
	'refreshSelectionPresentation',
	'removeFile',
	'reorderFiles',
	'selectAll',
	'selectFile',
	'setCurrentFileList',
	'setFileOrderLocked',
	'setOrderLocked',
	'setSelectedFileIndices',
	'setSelectedIndex',
	'setSortAscending',
	'showMultiSelection',
	'showSingleSelection',
	'stageMetadataToSelection',
	'toggleFileSort',
	'updateFileProperties',
] as const;

describe('File List Runtime public API contract', () => {
	beforeEach(() => {
		fileList.setCurrentFileList(null);
	});

	it('pins the file list public export strip', () => {
		expect(Object.keys(fileList).sort()).toEqual([...EXPECTED_FILE_LIST_EXPORTS].sort());
	});

	it('reads combined size through the public accessor', () => {
		expect(fileList.readCombinedSizeText()).toBe('--- MB');
	});
});
