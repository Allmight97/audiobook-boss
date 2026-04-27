import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fileImportUiState } from '../fileImport/state.svelte';

const context = vi.hoisted(() => ({
	openMock: vi.fn(),
	analyzeAudioFilesMock: vi.fn(),
	appendFileListMock: vi.fn(),
	persistPendingDraftsMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		open: context.openMock,
		analyzeAudioFiles: context.analyzeAudioFilesMock,
	},
}));

vi.mock('../fileList', () => ({
	appendFileList: context.appendFileListMock,
	persistPendingMetadataDraftsForCurrentSelection: context.persistPendingDraftsMock,
}));

vi.mock('../fileList/state', () => ({
	isOrderLocked: vi.fn(() => false),
}));

describe('file import handlers', () => {
	beforeEach(() => {
		context.openMock.mockReset();
		context.analyzeAudioFilesMock.mockReset();
		context.appendFileListMock.mockReset();
		context.persistPendingDraftsMock.mockReset();
		fileImportUiState.errorMessage = '';
		fileImportUiState.isDragOver = false;
		fileImportUiState.hasFiles = false;
	});

	it('aborts importing when metadata staging fails', async () => {
		context.openMock.mockResolvedValue(['/books/new.m4b']);
		context.analyzeAudioFilesMock.mockResolvedValue({
			files: [
				{
					path: '/books/new.m4b',
					size: 1,
					duration: 1,
					isValid: true,
					bitrate: 64,
					sampleRate: 44_100,
					channels: 2,
				},
			],
			totalDuration: 1,
			totalSize: 1,
			validCount: 1,
			invalidCount: 0,
		});
		context.persistPendingDraftsMock.mockResolvedValue(false);

		const { handleClickToSelect } = await import('../fileImport/handlers');
		await handleClickToSelect();

		expect(context.appendFileListMock).not.toHaveBeenCalled();
		expect(fileImportUiState.errorMessage).toBe(
			'Fix metadata validation errors before adding files.',
		);
	});
});
