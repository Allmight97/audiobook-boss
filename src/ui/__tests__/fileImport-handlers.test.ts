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
	onOrderLockChangeMock: vi.fn(),
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
	onOrderLockChange: context.onOrderLockChangeMock,
}));

async function flushAsync(): Promise<void> {
	for (let i = 0; i < 5; i += 1) {
		await Promise.resolve();
	}
	await new Promise((resolve) => setTimeout(resolve, 0));
}

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
		context.onOrderLockChangeMock.mockReset();
		context.onOrderLockChangeMock.mockReturnValue(() => undefined);
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
		await flushAsync();

		const openedHandler = listeners.get('opened-audio-files');
		expect(openedHandler).toBeDefined();
		await openedHandler?.({ payload: {} });
		await flushAsync();

		expect(context.discoverAudioImportPathsMock).toHaveBeenCalledWith(['/books/opened.m4b']);
		expect(context.analyzeAudioFilesMock).toHaveBeenCalledWith(['/books/opened.m4b']);
		expect(context.appendFileListMock).toHaveBeenCalled();
		dispose();
	});

	it('defers queued OS-opened files until import order unlocks', async () => {
		const listeners = new Map<string, (event: { payload: unknown }) => Promise<void> | void>();
		let orderLockListener: ((locked: boolean) => void) | undefined;
		context.listenMock.mockImplementation(
			async (event: string, handler: (event: { payload: unknown }) => Promise<void> | void) => {
				listeners.set(event, handler);
				return () => listeners.delete(event);
			},
		);
		context.onOrderLockChangeMock.mockImplementation((listener: (locked: boolean) => void) => {
			orderLockListener = listener;
			return () => {
				orderLockListener = undefined;
			};
		});
		context.isOrderLockedMock.mockReturnValue(true);
		context.takeOpenedAudioFilesMock.mockResolvedValue(['/books/opened-after-unlock.m4b']);
		context.analyzeAudioFilesMock.mockResolvedValue({
			files: [
				{
					path: '/books/opened-after-unlock.m4b',
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
		await flushAsync();

		const openedHandler = listeners.get('opened-audio-files');
		expect(openedHandler).toBeDefined();
		await openedHandler?.({ payload: {} });
		await flushAsync();

		expect(context.takeOpenedAudioFilesMock).not.toHaveBeenCalled();
		expect(fileImportUiState.errorMessage).toBe(
			'Order locked while processing. Wait for completion to add files.',
		);
		expect(orderLockListener).toBeDefined();

		context.isOrderLockedMock.mockReturnValue(false);
		orderLockListener?.(false);
		await flushAsync();

		expect(context.takeOpenedAudioFilesMock).toHaveBeenCalledTimes(1);
		expect(context.discoverAudioImportPathsMock).toHaveBeenCalledWith([
			'/books/opened-after-unlock.m4b',
		]);
		expect(context.appendFileListMock).toHaveBeenCalled();
		dispose();
	});
});
