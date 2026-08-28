import { fireEvent, render, screen } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../../types/audio';
import FileListIsland from '../FileListIsland.svelte';
import { setCurrentFileList, setSelectedFileIndices, setSelectedIndex } from '../state';

const context = vi.hoisted(() => ({
	clearAllFilesMock: vi.fn(),
	clearSelectionActionMock: vi.fn(),
	moveFileDownMock: vi.fn(),
	moveFileUpMock: vi.fn(),
	removeFileMock: vi.fn(),
	reorderFilesMock: vi.fn(),
	restoreImportOrderMock: vi.fn(),
	selectAllMock: vi.fn(),
	selectFileMock: vi.fn(async () => undefined),
	toggleFileSortMock: vi.fn(),
	metadataSaveInProgress: {
		subscribe: vi.fn((run: (value: boolean) => void) => {
			run(false);
			return () => {};
		}),
	},
}));

vi.mock('../actions', () => ({
	clearAllFiles: context.clearAllFilesMock,
	clearSelectionAction: context.clearSelectionActionMock,
	moveFileDown: context.moveFileDownMock,
	moveFileUp: context.moveFileUpMock,
	removeFile: context.removeFileMock,
	reorderFiles: context.reorderFilesMock,
	restoreImportOrder: context.restoreImportOrderMock,
	selectAll: context.selectAllMock,
	selectFile: context.selectFileMock,
	toggleFileSort: context.toggleFileSortMock,
}));

vi.mock('../../metadataSession', () => ({
	getMetadataForFile: vi.fn(() => undefined),
	getMetadataIntentPatchForFile: vi.fn(() => undefined),
	isUsableMetadataCache: vi.fn(() => false),
	metadataSaveInProgress: context.metadataSaveInProgress,
}));

vi.mock('../../remoteSource', () => ({
	hasSupplementalAssetsForInputId: vi.fn(() => false),
}));

function makeFileList(): FileListInfo {
	const files: AudioFile[] = [
		{ path: '/books/alpha.m4b', isValid: false, error: 'invalid test fixture' },
		{ path: '/books/bravo.m4b', isValid: false, error: 'invalid test fixture' },
	];

	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: 0,
		totalSize: 0,
		validCount: 0,
		invalidCount: files.length,
	};
}

function resetActionMocks(): void {
	context.clearAllFilesMock.mockClear();
	context.clearSelectionActionMock.mockClear();
	context.moveFileDownMock.mockClear();
	context.moveFileUpMock.mockClear();
	context.removeFileMock.mockClear();
	context.reorderFilesMock.mockClear();
	context.restoreImportOrderMock.mockClear();
	context.selectAllMock.mockClear();
	context.selectFileMock.mockClear();
	context.toggleFileSortMock.mockClear();
}

describe('FileList keyboard owner binding', () => {
	beforeEach(() => {
		setCurrentFileList(makeFileList());
		setSelectedIndex(-1);
		setSelectedFileIndices([]);
		resetActionMocks();
	});

	it('handles keyboard actions only from the focusable FileList region', async () => {
		render(FileListIsland);
		const listbox = screen.getByRole('listbox', { name: 'Audio files' });

		await fireEvent.keyDown(listbox, { key: 'ArrowDown' });
		expect(context.selectFileMock).toHaveBeenCalledWith(0, { multi: false, range: false });

		context.selectFileMock.mockClear();
		await fireEvent.keyDown(document.body, { key: 'ArrowDown' });
		await fireEvent.keyDown(window, { key: 'ArrowDown' });
		expect(context.selectFileMock).not.toHaveBeenCalled();

		await fireEvent.keyDown(listbox, { key: 'a', ctrlKey: true });
		expect(context.selectAllMock).toHaveBeenCalledTimes(1);

		context.clearSelectionActionMock.mockClear();
		await fireEvent.keyDown(document.body, { key: 'Escape' });
		await fireEvent.keyDown(window, { key: 'Escape' });
		expect(context.clearSelectionActionMock).not.toHaveBeenCalled();
	});
});
