import { describe, expect, it } from 'vitest';
import * as fileListSolid from '..';

const EXPECTED_SOLID_FILE_LIST_EXPORTS = [
	'clearFiles',
	'clearSelection',
	'coverThumbnailAtom',
	'fileListSessionAtom',
	'fileListViewAtom',
	'loadFileList',
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

describe('Solid File List public strip', () => {
	it('pins a small atom-and-action export surface', () => {
		expect(Object.keys(fileListSolid).sort()).toEqual([...EXPECTED_SOLID_FILE_LIST_EXPORTS].sort());
	});
});
