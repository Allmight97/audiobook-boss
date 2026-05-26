import { beforeEach, describe, expect, it, vi } from 'vitest';
import { fileImportUiState } from '../fileImport/state.svelte';

const context = vi.hoisted(() => ({
	getSupportedAudioImportMetadataMock: vi.fn(),
	listenMock: vi.fn(),
	openFilesMock: vi.fn(),
	openDirectoryMock: vi.fn(),
	discoverAudioImportPathsMock: vi.fn(),
	takeOpenedAudioFilesMock: vi.fn(),
	analyzeAudioFilesMock: vi.fn(),
	appendFileListMock: vi.fn(),
	persistPendingDraftsMock: vi.fn(),
	isOrderLockedMock: vi.fn(),
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		getSupportedAudioImportMetadata: context.getSupportedAudioImportMetadataMock,
		listen: context.listenMock,
		openFiles: context.openFilesMock,
		openDirectory: context.openDirectoryMock,
		discoverAudioImportPaths: context.discoverAudioImportPathsMock,
		takeOpenedAudioFiles: context.takeOpenedAudioFilesMock,
		analyzeAudioFiles: context.analyzeAudioFilesMock,
	},
}));

vi.mock('../fileList/actions', () => ({
	appendFileList: context.appendFileListMock,
	persistPendingMetadataDraftsForCurrentSelection: context.persistPendingDraftsMock,
}));

vi.mock('../fileList/state.svelte', () => ({
	getCurrentFileList: vi.fn(() => null),
	isOrderLocked: context.isOrderLockedMock,
}));

describe('file import handlers', () => {
	beforeEach(() => {
		context.getSupportedAudioImportMetadataMock.mockReset();
		context.getSupportedAudioImportMetadataMock.mockResolvedValue({
			formats: [],
			extensions: ['mp3', 'm4a', 'm4b', 'aac', 'wav', 'flac'],
			formatsText: 'MP3, M4A/M4B, AAC, WAV, and FLAC',
			supportText: 'Supports MP3, M4A/M4B, AAC, WAV, and FLAC audio files',
		});
		context.listenMock.mockReset();
		context.listenMock.mockResolvedValue(() => undefined);
		context.openFilesMock.mockReset();
		context.openDirectoryMock.mockReset();
		context.discoverAudioImportPathsMock.mockReset();
		context.discoverAudioImportPathsMock.mockImplementation(async (paths: string[]) => paths);
		context.takeOpenedAudioFilesMock.mockReset();
		context.takeOpenedAudioFilesMock.mockResolvedValue([]);
		context.analyzeAudioFilesMock.mockReset();
		context.appendFileListMock.mockReset();
		context.persistPendingDraftsMock.mockReset();
		context.isOrderLockedMock.mockReset();
		context.isOrderLockedMock.mockReturnValue(false);
		fileImportUiState.errorMessage = '';
		fileImportUiState.isDragOver = false;
		fileImportUiState.hasFiles = false;
	});

	it('aborts importing when metadata staging fails', async () => {
		context.openFilesMock.mockResolvedValue(['/books/new.m4b']);
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

	it('imports OS-opened files through the shared workflow', async () => {
		const listeners = new Map<string, (event: { payload: unknown }) => Promise<void> | void>();
		context.listenMock.mockImplementation(
			async (event: string, handler: (event: { payload: unknown }) => Promise<void> | void) => {
				listeners.set(event, handler);
				return () => listeners.delete(event);
			},
		);
		context.takeOpenedAudioFilesMock
			.mockResolvedValueOnce([])
			.mockResolvedValueOnce(['/books/opened.m4b']);
		context.analyzeAudioFilesMock.mockResolvedValue({
			files: [
				{
					path: '/books/opened.m4b',
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
		context.persistPendingDraftsMock.mockResolvedValue(true);

		const { attachTauriDragHandlers } = await import('../fileImport/handlers');
		const dispose = attachTauriDragHandlers({
			getCoverArtArea: () => null,
			getFileManagementContainer: () => null,
			getVisibleFiles: () => [],
		});
		await Promise.resolve();
		await Promise.resolve();

		const openedHandler = listeners.get('opened-audio-files');
		expect(openedHandler).toBeDefined();
		await openedHandler?.({ payload: {} });
		await Promise.resolve();
		await Promise.resolve();

		expect(context.discoverAudioImportPathsMock).toHaveBeenCalledWith(['/books/opened.m4b']);
		expect(context.analyzeAudioFilesMock).toHaveBeenCalledWith(['/books/opened.m4b']);
		expect(context.appendFileListMock).toHaveBeenCalled();
		dispose();
	});

	it('preserves queued OS-opened files while import order is locked', async () => {
		const listeners = new Map<string, (event: { payload: unknown }) => Promise<void> | void>();
		context.listenMock.mockImplementation(
			async (event: string, handler: (event: { payload: unknown }) => Promise<void> | void) => {
				listeners.set(event, handler);
				return () => listeners.delete(event);
			},
		);
		context.isOrderLockedMock.mockReturnValue(true);

		const { attachTauriDragHandlers } = await import('../fileImport/handlers');
		const dispose = attachTauriDragHandlers({
			getCoverArtArea: () => null,
			getFileManagementContainer: () => null,
			getVisibleFiles: () => [],
		});
		await Promise.resolve();
		await Promise.resolve();

		const openedHandler = listeners.get('opened-audio-files');
		expect(openedHandler).toBeDefined();
		await openedHandler?.({ payload: {} });
		await Promise.resolve();
		await Promise.resolve();

		expect(context.takeOpenedAudioFilesMock).not.toHaveBeenCalled();
		expect(fileImportUiState.errorMessage).toBe(
			'Order locked while processing. Wait for completion to add files.',
		);
		dispose();
	});
});
