import { render, waitFor } from '@testing-library/svelte';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TauriFileDropEvents } from '../../types/events';
import type { AcquisitionJob } from '../../types/remoteSource';
import FileImportIsland from '../fileImport/FileImportIsland.svelte';
import { clearFileImportError } from '../fileImport/state.svelte';
import { clearSelectionPanels } from '../fileList/metadataPanel';
import {
	getCurrentFileList,
	setCurrentFileList,
	setSelectedFileIndices,
	setSelectedIndex,
} from '../fileList/state.svelte';
import { fileListViewState } from '../fileList/viewState.svelte';
import { resetFileListViewState } from '../fileList/viewState.svelte';
import { clearMetadataState } from '../metadataState';
import {
	registerRemoteSourceSupplementalAssets,
	removeRemoteSourceSupplementalAssets,
} from '../remoteSource/sessionAssets.svelte';

type DragDropPayload = TauriFileDropEvents['tauri://drag-drop'];
type DragDropListener = (event: { payload: DragDropPayload }) => void;
type TauriListener = (event: { payload: unknown }) => void;

const {
	analyzeAudioFilesMock,
	discoverAudioImportPathsMock,
	getSupportedAudioImportMetadataMock,
	loadCoverArtFileMock,
	openDirectoryMock,
	openFilesMock,
	pushStatusPanelTransientStatusMock,
	readAudioMetadataMock,
	takeOpenedAudioFilesMock,
	listeners,
} = vi.hoisted(() => ({
	analyzeAudioFilesMock: vi.fn(),
	discoverAudioImportPathsMock: vi.fn(),
	getSupportedAudioImportMetadataMock: vi.fn(),
	loadCoverArtFileMock: vi.fn(),
	openDirectoryMock: vi.fn(),
	openFilesMock: vi.fn(),
	pushStatusPanelTransientStatusMock: vi.fn(),
	readAudioMetadataMock: vi.fn(),
	takeOpenedAudioFilesMock: vi.fn(),
	listeners: {} as Record<string, TauriListener>,
}));

vi.mock('../../lib/tauri/client', () => ({
	tauriClient: {
		listen: vi.fn((event: string, cb: TauriListener) => {
			listeners[event] = cb;
		}),
		getSupportedAudioImportMetadata: getSupportedAudioImportMetadataMock,
		discoverAudioImportPaths: discoverAudioImportPathsMock,
		takeOpenedAudioFiles: takeOpenedAudioFilesMock,
		openDirectory: openDirectoryMock,
		openFiles: openFilesMock,
		analyzeAudioFiles: analyzeAudioFilesMock,
		loadCoverArtFile: loadCoverArtFileMock,
		readAudioMetadata: readAudioMetadataMock,
	},
}));

vi.mock('../statusPanel', () => ({
	pushStatusPanelTransientStatus: pushStatusPanelTransientStatusMock,
}));

function fireDragDrop(position: { x: number; y: number }, paths: string[]) {
	const handler = listeners['tauri://drag-drop'] as DragDropListener | undefined;
	if (handler) {
		handler({ payload: { position, paths } });
	}
}

async function flushAsync(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
}

function makeAnalyzedFile(path: string, overrides: Partial<Record<string, unknown>> = {}) {
	return {
		path,
		isValid: true,
		duration: 1,
		size: 1000,
		bitrate: 64,
		sampleRate: 44100,
		channels: 2,
		format: 'mp3',
		...overrides,
	};
}

function makeAnalyzedFileList(
	files: ReturnType<typeof makeAnalyzedFile>[],
	overrides: Partial<{
		totalDuration: number;
		totalSize: number;
		validCount: number;
		invalidCount: number;
	}> = {},
) {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: overrides.totalDuration ?? files.length,
		totalSize: overrides.totalSize ?? files.length * 1000,
		validCount: overrides.validCount ?? files.filter((file) => file.isValid).length,
		invalidCount: overrides.invalidCount ?? files.filter((file) => file.isValid !== true).length,
	};
}

function acquisitionJobWithPdf(): AcquisitionJob {
	return {
		jobId: 'remote-job-1',
		providerId: 'audible',
		status: 'validated',
		progress: {
			stage: 'importHandoff',
			percentage: 100,
			message: 'Ready for import.',
			bytesDownloaded: undefined,
			bytesTotal: undefined,
			currentTitleId: 'B000000001',
			currentItemIndex: 1,
			totalItems: 1,
			terminal: true,
		},
		materializedFiles: [
			{
				inputId: 'provider-input-1',
				titleId: 'B000000001',
				path: '/tmp/book-with-pdf.m4b',
				sizeBytes: 1000,
				sha256: 'audio-sha',
			},
		],
		supplementalAssets: [
			{
				assetId: 'pdf-1',
				inputId: 'provider-input-1',
				titleId: 'B000000001',
				path: '/tmp/book-with-pdf.pdf',
				fileName: 'Supplemental PDF.pdf',
				sizeBytes: 32,
				sha256: 'pdf-sha',
			},
		],
		diagnostics: [],
	};
}

describe('File import drop vs cover art drop isolation', () => {
	beforeEach(() => {
		analyzeAudioFilesMock.mockReset();
		discoverAudioImportPathsMock.mockReset();
		discoverAudioImportPathsMock.mockImplementation(async (paths: string[]) => paths);
		getSupportedAudioImportMetadataMock.mockReset();
		getSupportedAudioImportMetadataMock.mockResolvedValue({
			formats: [],
			extensions: ['mp3', 'm4a', 'm4b', 'aac', 'wav', 'flac'],
			formatsText: 'MP3, M4A/M4B, AAC, WAV, and FLAC',
			supportText: 'Supports MP3, M4A/M4B, AAC, WAV, and FLAC audio files',
		});
		loadCoverArtFileMock.mockReset();
		loadCoverArtFileMock.mockResolvedValue(null);
		openDirectoryMock.mockReset();
		openFilesMock.mockReset();
		pushStatusPanelTransientStatusMock.mockReset();
		readAudioMetadataMock.mockReset();
		readAudioMetadataMock.mockResolvedValue({});
		takeOpenedAudioFilesMock.mockReset();
		takeOpenedAudioFilesMock.mockResolvedValue([]);
		clearMetadataState();
		clearFileImportError();
		removeRemoteSourceSupplementalAssets(['current-input-1']);
		setCurrentFileList(null);
		setSelectedFileIndices([]);
		setSelectedIndex(-1);
		clearSelectionPanels();
		resetFileListViewState();
		document.body.innerHTML = `
      <div id="cover-art-area"></div>
		`;
		render(FileImportIsland);

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
		openFilesMock.mockResolvedValue([]);

		const dropZone = document.querySelector('.drop-zone-header') as HTMLElement | null;
		expect(dropZone).toBeTruthy();
		dropZone?.click();
		await flushAsync();

		expect(openFilesMock).toHaveBeenCalledWith({
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

	it('uses backend-discovered files on drops over the file management container', async () => {
		discoverAudioImportPathsMock.mockResolvedValueOnce([
			'/tmp/file1.wav',
			'/tmp/file2.flac',
			'/tmp/file4.M4B',
			'/tmp/file5.Mp3',
		]);
		analyzeAudioFilesMock.mockResolvedValue(makeAnalyzedFileList([]));
		fireDragDrop({ x: 200, y: 200 }, [
			'/tmp/file1.wav',
			'/tmp/file2.flac',
			'/tmp/file3.txt',
			'/tmp/file4.M4B',
			'/tmp/file5.Mp3',
		]);
		await flushAsync();
		expect(discoverAudioImportPathsMock).toHaveBeenCalledWith([
			'/tmp/file1.wav',
			'/tmp/file2.flac',
			'/tmp/file3.txt',
			'/tmp/file4.M4B',
			'/tmp/file5.Mp3',
		]);
		expect(analyzeAudioFilesMock).toHaveBeenCalledWith([
			'/tmp/file1.wav',
			'/tmp/file2.flac',
			'/tmp/file4.M4B',
			'/tmp/file5.Mp3',
		]);
	});

	it('processes drops on file management container (file list area)', async () => {
		analyzeAudioFilesMock.mockResolvedValue(makeAnalyzedFileList([]));
		// Drop on file list content area (when files are present)
		fireDragDrop({ x: 200, y: 300 }, ['/tmp/file1.mp3']);
		await flushAsync();
		expect(analyzeAudioFilesMock).toHaveBeenCalledWith(['/tmp/file1.mp3']);
	});

	it('renders the supported audio copy from backend metadata', async () => {
		await flushAsync();
		expect(document.body.textContent).toContain(
			'Supports MP3, M4A/M4B, AAC, WAV, and FLAC audio files',
		);
	});

	it('ignores drops outside file management container', async () => {
		analyzeAudioFilesMock.mockResolvedValue(makeAnalyzedFileList([]));
		fireDragDrop({ x: 500, y: 500 }, ['/tmp/file1.mp3']);
		expect(analyzeAudioFilesMock).not.toHaveBeenCalled();
	});

	it('shows and wires the Clear button after files are loaded', async () => {
		analyzeAudioFilesMock.mockResolvedValue(
			makeAnalyzedFileList([makeAnalyzedFile('/tmp/file1.mp3')]),
		);

		fireDragDrop({ x: 200, y: 200 }, ['/tmp/file1.mp3']);
		await flushAsync();

		const clearButton = document.getElementById('clear-files-btn') as HTMLButtonElement | null;
		expect(clearButton).toBeTruthy();
		await waitFor(() => {
			expect(clearButton?.style.display).toBe('block');
			expect(document.querySelectorAll('.file-list-item')).toHaveLength(1);
		});

		clearButton?.click();
		await flushAsync();

		expect(document.querySelectorAll('.file-list-item')).toHaveLength(0);
		expect(clearButton?.style.display).toBe('none');
	});

	it('shows a PDF companion chip for imported files with acquired supplemental assets', async () => {
		analyzeAudioFilesMock.mockResolvedValue(
			makeAnalyzedFileList([
				makeAnalyzedFile('/tmp/book-with-pdf.m4b', {
					inputId: 'current-input-1',
				}),
			]),
		);

		fireDragDrop({ x: 200, y: 200 }, ['/tmp/book-with-pdf.m4b']);
		await waitFor(() => {
			expect(document.querySelectorAll('.file-list-item')).toHaveLength(1);
		});
		registerRemoteSourceSupplementalAssets(acquisitionJobWithPdf(), getCurrentFileList());

		await waitFor(() => {
			const chip = document.querySelector('.companion-chip') as HTMLElement | null;
			expect(chip?.textContent).toBe('PDF');
			expect(chip?.title).toBe('Supplemental PDF attached');
		});
	});

	it('appends a single new file to a populated list', async () => {
		analyzeAudioFilesMock.mockResolvedValueOnce(
			makeAnalyzedFileList([makeAnalyzedFile('/tmp/book-a.mp3')]),
		);
		fireDragDrop({ x: 200, y: 200 }, ['/tmp/book-a.mp3']);
		await flushAsync();

		await waitFor(() => {
			expect(document.querySelectorAll('.file-list-item')).toHaveLength(1);
		});

		openFilesMock.mockResolvedValueOnce(['/tmp/book-b.mp3']);
		analyzeAudioFilesMock.mockResolvedValueOnce(
			makeAnalyzedFileList([makeAnalyzedFile('/tmp/book-b.mp3')], {
				totalDuration: 2,
				totalSize: 2000,
			}),
		);

		document
			.querySelector('.drop-zone-header')
			?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
		await flushAsync();

		await waitFor(() => {
			expect(document.querySelectorAll('.file-list-item')).toHaveLength(2);
			expect(fileListViewState.files.map((file) => file.path)).toEqual([
				'/tmp/book-a.mp3',
				'/tmp/book-b.mp3',
			]);
		});
	});

	it('appends only unseen files when a mixed duplicate/new add arrives', async () => {
		analyzeAudioFilesMock.mockResolvedValueOnce(
			makeAnalyzedFileList([makeAnalyzedFile('/tmp/book-a.mp3')]),
		);
		fireDragDrop({ x: 200, y: 200 }, ['/tmp/book-a.mp3']);
		await flushAsync();

		analyzeAudioFilesMock.mockResolvedValueOnce(
			makeAnalyzedFileList(
				[makeAnalyzedFile('/tmp/book-a.mp3'), makeAnalyzedFile('/tmp/book-b.mp3')],
				{
					totalDuration: 2,
					totalSize: 2000,
					validCount: 2,
				},
			),
		);
		openFilesMock.mockResolvedValueOnce(['/tmp/book-a.mp3', '/tmp/book-b.mp3']);

		document
			.querySelector('.drop-zone-header')
			?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
		await flushAsync();

		await waitFor(() => {
			expect(document.querySelectorAll('.file-list-item')).toHaveLength(2);
			expect(fileListViewState.files.map((file) => file.path)).toEqual([
				'/tmp/book-a.mp3',
				'/tmp/book-b.mp3',
			]);
		});
	});

	it('leaves the list unchanged and surfaces status when only duplicates are added', async () => {
		analyzeAudioFilesMock.mockResolvedValueOnce(
			makeAnalyzedFileList([makeAnalyzedFile('/tmp/book-a.mp3')]),
		);
		fireDragDrop({ x: 200, y: 200 }, ['/tmp/book-a.mp3']);
		await waitFor(() => {
			expect(fileListViewState.files.map((file) => file.path)).toEqual(['/tmp/book-a.mp3']);
		});

		analyzeAudioFilesMock.mockResolvedValueOnce(
			makeAnalyzedFileList([makeAnalyzedFile('/tmp/book-a.mp3')]),
		);
		openFilesMock.mockResolvedValueOnce(['/tmp/book-a.mp3']);

		document
			.querySelector('.drop-zone-header')
			?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
		await flushAsync();

		await waitFor(() => {
			expect(document.querySelectorAll('.file-list-item')).toHaveLength(1);
			expect(fileListViewState.files.map((file) => file.path)).toEqual(['/tmp/book-a.mp3']);
		});
		expect(fileListViewState.files.map((file) => file.path)).toEqual(['/tmp/book-a.mp3']);
		await waitFor(() => {
			expect(document.querySelector('#file-import-error')?.textContent).toContain(
				'No new files added. All analyzed files were already in the list.',
			);
		});
		expect(pushStatusPanelTransientStatusMock).toHaveBeenCalledWith(
			'No new files added. All analyzed files were already in the list.',
			{ ttlMs: 2000 },
		);
	});
});
