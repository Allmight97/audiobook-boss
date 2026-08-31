import { cleanup, render, screen, waitFor } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo, SupportedAudioImportMetadata } from '../../types/audio';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import { fakeCoverCapability, JPEG_DATA_URL } from '../../test/fixtures/coverCapability';
import { App } from '../App';

const support: SupportedAudioImportMetadata = {
	formats: [{ extension: 'm4b', label: 'M4B' }],
	extensions: ['m4b'],
	formatsText: 'M4B',
	supportText: 'Supports M4B audio files',
};

function analyzedFile(path: string, title: string): FileListInfo['files'][number] {
	return {
		path,
		isValid: true,
		duration: 1,
		size: 1000,
		format: 'm4b',
		tagTitle: title,
		inputId: path,
	};
}

function analyzedList(files: FileListInfo['files']): FileListInfo {
	return {
		files,
		selectedDecoders: files.map(() => null),
		totalDuration: files.length,
		totalSize: files.length * 1000,
		validCount: files.length,
		invalidCount: 0,
	};
}

function fakeInput(overrides: Partial<InputCapability> = {}): InputCapability {
	return {
		openFiles: vi.fn(async () => ['/books/alpha.m4b']),
		openDirectory: vi.fn(async () => null),
		discoverAudioImportPaths: vi.fn(async (paths) => [...paths]),
		analyzeAudioFiles: vi.fn(async (paths: ReadonlyArray<string>) =>
			analyzedList(
				paths.map((path) => analyzedFile(path, path.includes('beta') ? 'Beta' : 'Alpha')),
			),
		),
		getSupportedAudioImportMetadata: vi.fn(async () => support),
		takeOpenedAudioFiles: vi.fn(async () => []),
		listenDragDrop: vi.fn(async () => () => undefined),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
		listenOpenedAudioFiles: vi.fn(async () => () => undefined),
		...overrides,
	};
}

function fakeMetadata(overrides: Partial<MetadataCapability> = {}): MetadataCapability {
	return {
		readAudioMetadata: vi.fn(async (filePath) => ({
			title: filePath.includes('beta') ? 'Beta' : 'Alpha',
			cover_art: [0x89, 0x50, 0x4e, 0x47],
		})),
		validateMetadataIntentPatch: vi.fn(async (patch) => ({
			isValid: true,
			metadataPatch: patch,
			fieldErrors: [],
		})),
		saveMetadataBatch: vi.fn(async (items: ReadonlyArray<{ filePath: string }>) => ({
			results: items.map((item, inputIndex) => ({
				inputIndex,
				filePath: item.filePath,
				status: 'success' as const,
				message: 'ok',
			})),
			summary: {
				succeeded: items.length,
				failed: 0,
				cancelled: 0,
				skipped: 0,
				total: items.length,
			},
		})),
		openFile: vi.fn(async () => '/covers/art.png'),
		searchOnlineMetadata: vi.fn(async () => ({ results: [], diagnostics: [] })),
		...overrides,
	};
}

function renderApp(runtime: AppRuntime) {
	return render(() => (
		<AppRuntimeProvider runtime={runtime}>
			<App />
		</AppRuntimeProvider>
	));
}

describe('metadata workbench shell', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	it('composes cover and form zones and keeps cover clear keyboard-reachable', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({
			input: fakeInput(),
			metadata,
			cover: fakeCoverCapability({
				thumbnail: vi.fn(async () => ({ handleId: null, dataUrl: JPEG_DATA_URL })),
			}),
		});
		renderApp(runtime);
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect(screen.getByTestId('metadata-manager')).toBeTruthy();
		});
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement | null)?.value).toBe(
				'Alpha',
			);
		});
		expect(screen.getByTestId('cover-art-area')).toBeTruthy();
		const loadButton = screen.getByTestId('cover-art-url-load-btn');
		const findMetadata = screen.getByTestId('metadata-lookup-btn');
		expect(loadButton.className.split(/\s+/)).toEqual(
			expect.arrayContaining(['abb-button', 'abb-button-secondary', 'cover-art-url-load-btn']),
		);
		expect(findMetadata.className.split(/\s+/)).toEqual(
			expect.arrayContaining(['abb-button', 'abb-button-secondary']),
		);
		expect(screen.queryByTestId('metadata-artifacts')).toBeNull();
		const clearButton = document.getElementById('cover-art-clear-btn') as HTMLButtonElement;
		expect(clearButton.tabIndex).toBe(0);
		expect(getComputedStyle(clearButton).display).not.toBe('none');
		clearButton.focus();
		expect(document.activeElement).toBe(clearButton);
	});

	it('edits a title and saves through the native metadata capability', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({
			input: fakeInput(),
			metadata,
			cover: fakeCoverCapability({
				thumbnail: vi.fn(async () => ({ handleId: null, dataUrl: JPEG_DATA_URL })),
			}),
		});
		renderApp(runtime);
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement).value).toBe('Alpha');
		});
		const title = document.getElementById('meta-title') as HTMLInputElement;
		title.focus();
		await userEvent.clear(title);
		await userEvent.type(title, 'Edited');
		await userEvent.click(screen.getByTestId('metadata-save-btn'));
		await waitFor(() => {
			expect(metadata.saveMetadataBatch).toHaveBeenCalled();
		});
		expect(metadata.validateMetadataIntentPatch).toHaveBeenCalled();
	});

	it('saves from the global shortcut', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({
			input: fakeInput(),
			metadata,
			cover: fakeCoverCapability({
				thumbnail: vi.fn(async () => ({ handleId: null, dataUrl: JPEG_DATA_URL })),
			}),
		});
		renderApp(runtime);
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => {
			expect((document.getElementById('meta-title') as HTMLInputElement).value).toBe('Alpha');
		});
		const title = document.getElementById('meta-title') as HTMLInputElement;
		await userEvent.type(title, ' Two');
		window.dispatchEvent(new KeyboardEvent('keydown', { key: 's', metaKey: true, bubbles: true }));
		await waitFor(() => {
			expect(metadata.saveMetadataBatch).toHaveBeenCalled();
		});
	});
});
