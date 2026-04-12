import { render } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TauriFileDropEvents } from '../../types/events';
import FileImportIsland from '../fileImport/FileImportIsland.svelte';
import { initFileImport } from '../fileImport';
import { SUPPORTED_AUDIO_SUPPORT_TEXT } from '../fileImport/supportedAudio';

type DragDropPayload = TauriFileDropEvents['tauri://drag-drop'];
type DragDropListener = (event: { payload: DragDropPayload }) => void;

const { analyzeAudioFilesMock, loadCoverArtFileMock, openMock, readAudioMetadataMock, listeners } =
	vi.hoisted(() => ({
		analyzeAudioFilesMock: vi.fn(),
		loadCoverArtFileMock: vi.fn(),
		openMock: vi.fn(),
		readAudioMetadataMock: vi.fn(),
		listeners: {} as Record<'tauri://drag-drop', DragDropListener>,
	}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		listen: vi.fn((event: string, cb: DragDropListener) => {
			if (event === 'tauri://drag-drop') {
				listeners[event] = cb;
			}
		}),
		open: openMock,
		analyzeAudioFiles: analyzeAudioFilesMock,
		loadCoverArtFile: loadCoverArtFileMock,
		readAudioMetadata: readAudioMetadataMock,
	},
}));

function fireDragDrop(position: { x: number; y: number }, paths: string[]) {
	const handler = listeners['tauri://drag-drop'];
	if (handler) {
		handler({ payload: { position, paths } });
	}
}

async function flushAsync(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
}

describe('File import drop vs cover art drop isolation', () => {
	beforeEach(() => {
		analyzeAudioFilesMock.mockReset();
		loadCoverArtFileMock.mockReset();
		loadCoverArtFileMock.mockResolvedValue(null);
		openMock.mockReset();
		readAudioMetadataMock.mockReset();
		readAudioMetadataMock.mockResolvedValue({});
		document.body.innerHTML = `
      <div id="cover-art-area"></div>
    `;
		render(FileImportIsland);
		initFileImport();

		const dropZone = document.querySelector('.drop-zone-header') as HTMLElement | null;
		if (!dropZone) {
			throw new Error('Expected file import island to render drop zone');
		}

		const container = document.querySelector('.file-management-container') as HTMLElement | null;
		if (!container) {
			throw new Error('Expected file import island to render file management container');
		}

		// Mock cover art bounds
		const cover = document.getElementById('cover-art-area') as HTMLElement;
		cover.getBoundingClientRect = () =>
			({
				left: 0,
				right: 100,
				top: 0,
				bottom: 100,
				width: 100,
				height: 100,
				x: 0,
				y: 0,
				toJSON: () => ({}),
			}) as DOMRect;

		// Mock file management container bounds (entire drop area)
		container.getBoundingClientRect = () =>
			({
				left: 150,
				right: 400,
				top: 150,
				bottom: 350,
				width: 250,
				height: 200,
				x: 150,
				y: 150,
				toJSON: () => ({}),
			}) as DOMRect;
	});

	it('uses the expanded supported audio list in the file picker filter', async () => {
		openMock.mockResolvedValue([]);

		const dropZone = document.querySelector('.drop-zone-header') as HTMLElement | null;
		expect(dropZone).toBeTruthy();
		dropZone?.click();
		await flushAsync();

		expect(openMock).toHaveBeenCalledWith({
			multiple: true,
			directory: false,
			filters: [
				{
					name: 'Audio Files',
					extensions: ['mp3', 'm4a', 'm4b', 'aac', 'wav', 'flac'],
				},
			],
		});
	});

	it('ignores drops inside cover art area', async () => {
		fireDragDrop({ x: 50, y: 50 }, ['/tmp/image.png']);
		expect(analyzeAudioFilesMock).not.toHaveBeenCalled();
	});

	it('filters supported files on drops over the file management container', async () => {
		analyzeAudioFilesMock.mockResolvedValue({
			files: [],
			totalDuration: 0,
			totalSize: 0,
			validCount: 0,
			invalidCount: 0,
		});
		fireDragDrop({ x: 200, y: 200 }, ['/tmp/file1.wav', '/tmp/file2.flac', '/tmp/file3.txt']);
		expect(analyzeAudioFilesMock).toHaveBeenCalledWith(['/tmp/file1.wav', '/tmp/file2.flac']);
	});

	it('processes drops on file management container (file list area)', async () => {
		analyzeAudioFilesMock.mockResolvedValue({
			files: [],
			totalDuration: 0,
			totalSize: 0,
			validCount: 0,
			invalidCount: 0,
		});
		// Drop on file list content area (when files are present)
		fireDragDrop({ x: 200, y: 300 }, ['/tmp/file1.mp3']);
		expect(analyzeAudioFilesMock).toHaveBeenCalledWith(['/tmp/file1.mp3']);
	});

	it('renders the supported audio copy from the shared source', () => {
		expect(document.body.textContent).toContain(SUPPORTED_AUDIO_SUPPORT_TEXT);
	});

	it('ignores drops outside file management container', async () => {
		analyzeAudioFilesMock.mockResolvedValue({
			files: [],
			totalDuration: 0,
			totalSize: 0,
			validCount: 0,
			invalidCount: 0,
		});
		fireDragDrop({ x: 500, y: 500 }, ['/tmp/file1.mp3']);
		expect(analyzeAudioFilesMock).not.toHaveBeenCalled();
	});

	it('shows and wires the Clear button after files are loaded', async () => {
		analyzeAudioFilesMock.mockResolvedValue({
			files: [
				{
					path: '/tmp/file1.mp3',
					isValid: true,
					duration: 1,
					size: 1000,
					bitrate: 64,
					sampleRate: 44100,
					channels: 2,
					format: 'mp3',
				},
			],
			totalDuration: 1,
			totalSize: 1000,
			validCount: 1,
			invalidCount: 0,
		});

		fireDragDrop({ x: 200, y: 200 }, ['/tmp/file1.mp3']);
		await flushAsync();

		const clearButton = document.getElementById('clear-files-btn') as HTMLButtonElement | null;
		expect(clearButton).toBeTruthy();
		expect(clearButton?.style.display).toBe('block');
		expect(document.querySelectorAll('.file-list-item')).toHaveLength(1);

		clearButton?.click();
		await flushAsync();

		expect(document.querySelectorAll('.file-list-item')).toHaveLength(0);
		expect(clearButton?.style.display).toBe('none');
	});
});
