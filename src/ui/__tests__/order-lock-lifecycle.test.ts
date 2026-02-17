import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../types/audio';
import {
	isOrderLocked,
	setOrderLocked,
	setCurrentFileList,
	getCurrentFileList,
} from '../fileList/state';

// Mock modules that actions.ts imports but aren't relevant to lock behavior
vi.mock('../metadataForm', () => ({
	hasDirtyMetadataFields: vi.fn(() => false),
	readMetadataForm: vi.fn(() => ({})),
	resetDirtyState: vi.fn(),
}));

vi.mock('../metadataState', () => ({
	clearMetadataState: vi.fn(),
	getMetadataForFile: vi.fn(() => ({})),
	removeMetadataForFile: vi.fn(),
	setMetadataForFile: vi.fn(),
}));

vi.mock('../outputPanel', () => ({
	onFileListChange: vi.fn(),
	onMetadataChange: vi.fn(),
}));

vi.mock('../metadataValidation', () => ({
	getSeriesPartValidationError: vi.fn(() => null),
	getSubseriesPartValidationError: vi.fn(() => null),
}));

vi.mock('../fileList/events', () => ({
	initFileListEvents: vi.fn(),
	setupDragStartHandlers: vi.fn(),
}));

vi.mock('../fileList/selection', () => ({
	clearSelection: vi.fn(() => true),
	handleSelection: vi.fn(() => ({ changed: true })),
	reindexSelectionAfterMove: vi.fn(),
	reindexSelectionAfterRemoval: vi.fn(),
	selectAllFiles: vi.fn(() => true),
	swapSelectionIndices: vi.fn(),
}));

vi.mock('../fileList/metadataPanel', () => ({
	autoUpdateCoverArtFromFirstValidFile: vi.fn(async () => undefined),
	clearSelectionPanels: vi.fn(),
	ensureMetadataForFiles: vi.fn(async () => undefined),
	getSelectedFiles: vi.fn(() => []),
	showMultiSelection: vi.fn(async () => undefined),
	showSingleSelection: vi.fn(async () => undefined),
}));

vi.mock('../fileImport', () => ({
	updateDropZoneState: vi.fn(),
}));

function makeFileList(count: number): FileListInfo {
	return {
		files: Array.from({ length: count }, (_, i) => ({
			path: `/tmp/file-${i}.m4b`,
			isValid: true,
			size: 1000,
			duration: 300,
			bitrate: 64,
			sampleRate: 44100,
			channels: 2,
			format: 'M4B',
		})),
		totalDuration: count * 300,
		totalSize: count * 1000,
		validCount: count,
		invalidCount: 0,
	};
}

function setupDOM(): void {
	document.body.innerHTML = `
		<div class="file-list-content"></div>
		<button id="sort-toggle-btn">Sort: A-Z</button>
		<button id="clear-files-btn" style="display: block">Clear Files</button>
		<div id="file-order-lock" style="display: none"></div>
	`;
}

describe('order lock lifecycle', () => {
	beforeEach(() => {
		// Reset lock state between tests
		setOrderLocked(false);
		setCurrentFileList(null);
		setupDOM();
	});

	describe('state transitions', () => {
		it('starts unlocked', () => {
			expect(isOrderLocked()).toBe(false);
		});

		it('locks when setFileOrderLocked(true) is called', async () => {
			const { setFileOrderLocked } = await import('../fileList/actions');
			setCurrentFileList(makeFileList(2));
			setFileOrderLocked(true);
			expect(isOrderLocked()).toBe(true);
		});

		it('unlocks when setFileOrderLocked(false) is called', async () => {
			const { setFileOrderLocked } = await import('../fileList/actions');
			setCurrentFileList(makeFileList(2));
			setFileOrderLocked(true);
			expect(isOrderLocked()).toBe(true);
			setFileOrderLocked(false);
			expect(isOrderLocked()).toBe(false);
		});

		it('is idempotent — double lock/unlock does not break state', async () => {
			const { setFileOrderLocked } = await import('../fileList/actions');
			setCurrentFileList(makeFileList(2));
			setFileOrderLocked(true);
			setFileOrderLocked(true);
			expect(isOrderLocked()).toBe(true);
			setFileOrderLocked(false);
			setFileOrderLocked(false);
			expect(isOrderLocked()).toBe(false);
		});
	});

	describe('guard behavior — locked prevents mutations', () => {
		it('clearAllFiles() no-ops when locked', async () => {
			const { clearAllFiles, setFileOrderLocked } = await import('../fileList/actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			setFileOrderLocked(true);

			clearAllFiles();

			const current = getCurrentFileList();
			expect(current?.files.length).toBe(3);
		});

		it('removeFile() no-ops when locked', async () => {
			const { removeFile, setFileOrderLocked } = await import('../fileList/actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			setFileOrderLocked(true);

			removeFile(0);

			const current = getCurrentFileList();
			expect(current?.files.length).toBe(3);
		});

		it('moveFileUp() no-ops when locked', async () => {
			const { moveFileUp, setFileOrderLocked } = await import('../fileList/actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			const originalOrder = fileList.files.map((f) => f.path);
			setFileOrderLocked(true);

			moveFileUp(1);

			const current = getCurrentFileList();
			expect(current?.files.map((f) => f.path)).toEqual(originalOrder);
		});

		it('moveFileDown() no-ops when locked', async () => {
			const { moveFileDown, setFileOrderLocked } = await import('../fileList/actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			const originalOrder = fileList.files.map((f) => f.path);
			setFileOrderLocked(true);

			moveFileDown(0);

			const current = getCurrentFileList();
			expect(current?.files.map((f) => f.path)).toEqual(originalOrder);
		});

		it('toggleFileSort() no-ops when locked', async () => {
			const { toggleFileSort, setFileOrderLocked } = await import('../fileList/actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			const originalOrder = fileList.files.map((f) => f.path);
			setFileOrderLocked(true);

			toggleFileSort();

			const current = getCurrentFileList();
			expect(current?.files.map((f) => f.path)).toEqual(originalOrder);
		});

		it('reorderFiles() no-ops when locked', async () => {
			const { reorderFiles, setFileOrderLocked } = await import('../fileList/actions');
			const fileList = makeFileList(3);
			setCurrentFileList(fileList);
			const originalOrder = fileList.files.map((f) => f.path);
			setFileOrderLocked(true);

			reorderFiles(0, 2);

			const current = getCurrentFileList();
			expect(current?.files.map((f) => f.path)).toEqual(originalOrder);
		});
	});

	describe('DOM state reflects lock', () => {
		it('disables sort and clear buttons when locked', async () => {
			const { initDOMCache, updateButtonVisibility } = await import('../fileList/dom');
			setCurrentFileList(makeFileList(3));
			initDOMCache();
			setOrderLocked(true);

			updateButtonVisibility();

			const sortBtn = document.getElementById('sort-toggle-btn') as HTMLButtonElement;
			const clearBtn = document.getElementById('clear-files-btn') as HTMLButtonElement;
			expect(sortBtn.disabled).toBe(true);
			expect(clearBtn.disabled).toBe(true);
		});

		it('enables sort and clear buttons when unlocked', async () => {
			const { initDOMCache, updateButtonVisibility } = await import('../fileList/dom');
			setCurrentFileList(makeFileList(3));
			initDOMCache();
			setOrderLocked(false);

			updateButtonVisibility();

			const sortBtn = document.getElementById('sort-toggle-btn') as HTMLButtonElement;
			const clearBtn = document.getElementById('clear-files-btn') as HTMLButtonElement;
			expect(sortBtn.disabled).toBe(false);
			expect(clearBtn.disabled).toBe(false);
		});

		it('shows lock notice when locked', async () => {
			const { initDOMCache, setOrderLockNotice } = await import('../fileList/dom');
			initDOMCache();

			setOrderLockNotice(true);

			const notice = document.getElementById('file-order-lock')!;
			expect(notice.style.display).toBe('inline');
		});

		it('hides lock notice when unlocked', async () => {
			const { initDOMCache, setOrderLockNotice } = await import('../fileList/dom');
			initDOMCache();
			setOrderLockNotice(true);

			setOrderLockNotice(false);

			const notice = document.getElementById('file-order-lock')!;
			expect(notice.style.display).toBe('none');
		});

		it('renders file items as non-draggable when locked', async () => {
			const { createFileListItem } = await import('../fileList/dom');
			setCurrentFileList(makeFileList(2));
			setOrderLocked(true);

			const item = createFileListItem(getCurrentFileList()!.files[0], 0);

			expect(item.getAttribute('draggable')).toBe('false');
		});

		it('renders file items as draggable when unlocked', async () => {
			const { createFileListItem } = await import('../fileList/dom');
			setCurrentFileList(makeFileList(2));
			setOrderLocked(false);

			const item = createFileListItem(getCurrentFileList()!.files[0], 0);

			expect(item.getAttribute('draggable')).toBe('true');
		});

		it('disables move and remove buttons in file items when locked', async () => {
			const { createFileListItem } = await import('../fileList/dom');
			setCurrentFileList(makeFileList(2));
			setOrderLocked(true);

			const item = createFileListItem(getCurrentFileList()!.files[1], 1);

			const moveUp = item.querySelector('.move-up-btn') as HTMLButtonElement;
			const moveDown = item.querySelector('.move-down-btn') as HTMLButtonElement;
			const remove = item.querySelector('.remove-file-btn') as HTMLButtonElement;
			expect(moveUp.disabled).toBe(true);
			expect(moveDown.disabled).toBe(true);
			expect(remove.disabled).toBe(true);
		});
	});

	describe('round-trip: lock → unlock restores full interactivity', () => {
		it('mutations work after lock/unlock cycle', async () => {
			const { setFileOrderLocked, clearAllFiles } = await import('../fileList/actions');
			setCurrentFileList(makeFileList(3));

			// Lock then unlock
			setFileOrderLocked(true);
			setFileOrderLocked(false);

			// clearAllFiles should now work
			clearAllFiles();

			const current = getCurrentFileList();
			expect(current?.files.length).toBe(0);
		});

		it('DOM buttons re-enabled after lock/unlock cycle', async () => {
			const { initDOMCache, updateButtonVisibility } = await import('../fileList/dom');
			setCurrentFileList(makeFileList(3));
			initDOMCache();

			// Lock
			setOrderLocked(true);
			updateButtonVisibility();
			const clearBtn = document.getElementById('clear-files-btn') as HTMLButtonElement;
			expect(clearBtn.disabled).toBe(true);

			// Unlock
			setOrderLocked(false);
			updateButtonVisibility();
			expect(clearBtn.disabled).toBe(false);
		});
	});
});
