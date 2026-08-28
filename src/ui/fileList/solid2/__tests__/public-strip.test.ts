import { describe, expect, it } from 'vitest';
import * as fileListSolid2 from '..';

const EXPECTED_SOLID2_FILE_LIST_EXPORTS = [
	'FileListIsland',
	'clearFiles',
	'clearSelection',
	'getFileListCoverThumbnailState',
	'loadFileList',
	'readFileListView',
	'removeFile',
	'reorderFiles',
	'resetFileList',
	'restoreImportOrder',
	'selectAll',
	'selectFile',
	'setCoverThumbnailLoader',
	'setOrderLocked',
	'toggleSort',
] as const;

describe('Solid 2 File List public strip', () => {
	it('pins a small action-and-read export surface', () => {
		expect(Object.keys(fileListSolid2).sort()).toEqual(
			[...EXPECTED_SOLID2_FILE_LIST_EXPORTS].sort(),
		);
	});
});
