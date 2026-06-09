import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { AudioFile, FileListInfo } from '../../types/audio';
import { getCurrentCoverArt, refreshCoverArtDisplay, setCustomCoverArt } from '../coverArt';
import { jobControlsState } from '../jobControls/state.svelte';
import {
	setCurrentFileList,
	setSelectedFileIndices,
	setSelectedIndex,
} from '../fileList/state.svelte';
import {
	clearMetadataState,
	getMetadataIntentPatchForFile,
	setMetadataForFile,
} from '../metadataState';

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		listen: vi.fn(),
	},
}));

function makeFile(path: string): AudioFile {
	return { path, isValid: true, inputId: path } as AudioFile;
}

function makeFileList(files: AudioFile[]): FileListInfo {
	return {
		files,
		validCount: files.length,
		invalidCount: 0,
		totalDuration: 0,
		totalSize: 0,
	} as FileListInfo;
}

describe('coverArt owner integration', () => {
	beforeEach(() => {
		clearMetadataState();
		jobControlsState.jobType = 'batch';
		setCurrentFileList(makeFileList([makeFile('/books/a.m4b'), makeFile('/books/b.m4b')]));
		setSelectedFileIndices([0]);
		setSelectedIndex(0);
	});

	it('commits custom cover art to the selected batch file only', () => {
		setSelectedFileIndices([1]);
		setSelectedIndex(1);

		setCustomCoverArt([4, 5, 6]);

		expect(getMetadataIntentPatchForFile('/books/b.m4b')?.cover_art).toEqual({
			op: 'set',
			value: [4, 5, 6],
		});
		expect(getMetadataIntentPatchForFile('/books/a.m4b')?.cover_art).toBeUndefined();
		expect(getCurrentCoverArt()).toEqual([4, 5, 6]);
	});

	it('commits merge cover art to the first valid file regardless of selection', () => {
		jobControlsState.jobType = 'merge';
		setSelectedFileIndices([1]);
		setSelectedIndex(1);

		setCustomCoverArt([1, 2, 3]);

		expect(getMetadataIntentPatchForFile('/books/a.m4b')?.cover_art).toEqual({
			op: 'set',
			value: [1, 2, 3],
		});
		expect(getMetadataIntentPatchForFile('/books/b.m4b')?.cover_art).toBeUndefined();
	});

	it('refreshes batch preview when cycling selected files', () => {
		setMetadataForFile('/books/a.m4b', { cover_art: [1] });
		setMetadataForFile('/books/b.m4b', { cover_art: [2] });

		setSelectedFileIndices([0]);
		setSelectedIndex(0);
		refreshCoverArtDisplay();
		expect(getCurrentCoverArt()).toEqual([1]);

		setSelectedFileIndices([1]);
		setSelectedIndex(1);
		refreshCoverArtDisplay();
		expect(getCurrentCoverArt()).toEqual([2]);
	});
});
