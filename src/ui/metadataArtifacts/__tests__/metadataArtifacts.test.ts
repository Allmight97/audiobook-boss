import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../../types/audio';

const context = vi.hoisted(() => ({
	getCurrentFileListMock: vi.fn(),
	getSelectedFileIndicesMock: vi.fn(),
}));

vi.mock('../../fileList', () => ({
	getCurrentFileList: context.getCurrentFileListMock,
	getSelectedFileIndices: context.getSelectedFileIndicesMock,
}));

import {
	clearMetadataState,
	getMetadataIntentPatchForFile,
	getPendingMetadataIntentEntries,
	setMetadataForFile,
} from '../../metadataState';
import {
	metadataArtifactsState,
	refreshMetadataArtifacts,
	stageMetadataArtifactClear,
} from '../state.svelte';

const FILE_PATH = '/books/feedback.m4b';

function fileListWithOneValidFile(): FileListInfo {
	return {
		files: [{ path: FILE_PATH, isValid: true }],
		selectedDecoders: [null],
		totalDuration: 0,
		totalSize: 0,
		validCount: 1,
		invalidCount: 0,
	} as FileListInfo;
}

describe('metadata artifacts inspect/clear', () => {
	beforeEach(() => {
		vi.clearAllMocks();
		clearMetadataState();
		context.getCurrentFileListMock.mockReturnValue(fileListWithOneValidFile());
		context.getSelectedFileIndicesMock.mockReturnValue(new Set([0]));
	});

	it('shows artifact values for the selected file, distinct from primary fields', () => {
		setMetadataForFile(FILE_PATH, {
			title: 'Feedback',
			album_sort: 'test series 01 - Feedback',
			comment: 'Provenance note',
			track: [7, 42],
			disk: [1, undefined as unknown as number],
		});

		refreshMetadataArtifacts();

		expect(metadataArtifactsState.filePath).toBe(FILE_PATH);
		const byField = Object.fromEntries(
			metadataArtifactsState.rows.map((row) => [row.field, row.value]),
		);
		expect(byField.album_sort).toBe('test series 01 - Feedback');
		expect(byField.comment).toBe('Provenance note');
		expect(byField.track).toBe('7 of 42');
		expect(byField.disk).toBe('1');
		// Primary fields never appear as artifact rows.
		expect(metadataArtifactsState.rows.map((row) => row.field)).not.toContain('title');
	});

	it('stages an explicit clear intent per artifact field via the pending-save mechanism', () => {
		setMetadataForFile(FILE_PATH, {
			album_sort: 'test series 01 - Feedback',
			comment: 'Provenance note',
		});
		refreshMetadataArtifacts();

		stageMetadataArtifactClear('album_sort');

		const patch = getMetadataIntentPatchForFile(FILE_PATH);
		expect(patch?.album_sort).toEqual({ op: 'clear' });
		expect(patch?.comment).toBeUndefined();
		const pending = getPendingMetadataIntentEntries();
		expect(pending.map(([path]) => path)).toContain(FILE_PATH);
		const row = metadataArtifactsState.rows.find((entry) => entry.field === 'album_sort');
		expect(row?.clearPending).toBe(true);
	});

	it('stages track and disk clears with the same explicit intent shape', () => {
		setMetadataForFile(FILE_PATH, { track: [3, 12], disk: [1, 2] });
		refreshMetadataArtifacts();

		stageMetadataArtifactClear('track');
		stageMetadataArtifactClear('disk');

		const patch = getMetadataIntentPatchForFile(FILE_PATH);
		expect(patch?.track).toEqual({ op: 'clear' });
		expect(patch?.disk).toEqual({ op: 'clear' });
	});

	it('leaves untouched artifacts out of the pending intent entirely', () => {
		setMetadataForFile(FILE_PATH, {
			comment: 'Keep me',
			track: [3, 12],
		});
		refreshMetadataArtifacts();

		stageMetadataArtifactClear('comment');

		const patch = getMetadataIntentPatchForFile(FILE_PATH);
		expect(patch?.comment).toEqual({ op: 'clear' });
		// No intent op at all for the preserved field: normal saves keep it.
		expect(patch?.track).toBeUndefined();
		expect(patch?.disk).toBeUndefined();
	});

	it('reports multi-selection instead of showing artifact rows', () => {
		context.getSelectedFileIndicesMock.mockReturnValue(new Set([0, 1]));

		refreshMetadataArtifacts();

		expect(metadataArtifactsState.filePath).toBeNull();
		expect(metadataArtifactsState.rows).toEqual([]);
		expect(metadataArtifactsState.multiSelection).toBe(true);
	});

	it('treats zero track positions as already absent', () => {
		setMetadataForFile(FILE_PATH, { track: [0, 5] });

		refreshMetadataArtifacts();

		const row = metadataArtifactsState.rows.find((entry) => entry.field === 'track');
		expect(row?.value).toBeNull();
	});
});
