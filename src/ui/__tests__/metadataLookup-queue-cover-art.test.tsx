import { cleanup, render, waitFor } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo, SupportedAudioImportMetadata } from '../../types/audio';
import { getMetadataForFile } from '../../app/metadataSession';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
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

function lookupResult(coverUrl: string | undefined = 'https://example.com/cover.jpg') {
	return {
		source: 'audnexus' as const,
		sourceId: 'audnexus:1',
		title: 'Lookup Title',
		authors: ['Author One'],
		narrators: ['Narrator One'],
		series: undefined,
		seriesPart: undefined,
		subseries: undefined,
		subseriesPart: undefined,
		description: 'Description',
		publishedDate: '2020-07',
		durationSeconds: 3600,
		audibleOnly: false,
		coverUrl,
	};
}

function fakeInput(): InputCapability {
	return {
		openFiles: vi.fn(async () => ['/books/alpha.m4b', '/books/beta.m4b']),
		openDirectory: vi.fn(async () => null),
		discoverAudioImportPaths: vi.fn(async (paths) => [...paths]),
		analyzeAudioFiles: vi.fn(async (paths: ReadonlyArray<string>) =>
			analyzedList(
				paths.map((path) => analyzedFile(path, path.includes('beta') ? 'Beta' : 'Alpha')),
			),
		),
		getSupportedAudioImportMetadata: vi.fn(async () => support),
		takeOpenedAudioFiles: vi.fn(async () => []),
		readAudioCoverThumbnail: vi.fn(async () => null),
		listenDragDrop: vi.fn(async () => () => undefined),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
		listenOpenedAudioFiles: vi.fn(async () => () => undefined),
	};
}

function fakeMetadata(overrides: Partial<MetadataCapability> = {}): MetadataCapability {
	return {
		readAudioMetadata: vi.fn(async (filePath) => ({
			title: filePath.includes('beta') ? 'Beta Existing' : 'Alpha Existing',
			cover_art: filePath.includes('beta') ? [2, 2, 2] : [1, 1, 1],
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
		openFile: vi.fn(async () => null),
		loadCoverArtFile: vi.fn(async () => []),
		loadCoverArtFromUrl: vi.fn(async () => [9, 9, 9]),
		searchOnlineMetadata: vi.fn(async () => ({
			results: [lookupResult()],
			diagnostics: [],
		})),
		...overrides,
	};
}

function getStatusText(): string {
	return (document.getElementById('metadata-lookup-status') as HTMLElement).textContent ?? '';
}

function getContextText(): string {
	return (document.getElementById('metadata-lookup-context') as HTMLElement).textContent ?? '';
}

async function importAndSelectAll(runtime: AppRuntime): Promise<void> {
	const addButton = document.querySelector<HTMLButtonElement>('[aria-label="Add audio files"]');
	if (!addButton) {
		throw new Error('Add audio files button missing');
	}
	await userEvent.click(addButton);
	await waitFor(() => {
		expect(runtime.input.view().files.length).toBe(2);
	});
	runtime.input.selectAll();
}

describe('metadata lookup queue cover art isolation', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	it('does not wipe previously replaced art when queue advances', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({ input: fakeInput(), metadata });
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<App />
			</AppRuntimeProvider>
		));
		await importAndSelectAll(runtime);
		await userEvent.click(document.getElementById('metadata-lookup-btn') as HTMLElement);
		await waitFor(() => {
			expect(document.getElementById('metadata-lookup-modal')?.classList.contains('open')).toBe(
				true,
			);
		});
		const toggle = document.getElementById('metadata-lookup-cover-toggle') as HTMLInputElement;
		await userEvent.click(toggle);
		const apply = await waitFor(() => {
			const button = document.querySelector<HTMLButtonElement>(
				"#metadata-lookup-results button[data-index='0']",
			);
			if (!button) throw new Error('Expected an apply button');
			return button;
		});
		await userEvent.click(apply);
		await waitFor(() => {
			expect(getStatusText()).toContain('Metadata applied.');
		});
		expect(getContextText()).toContain('beta.m4b');
		expect(metadata.loadCoverArtFromUrl).toHaveBeenCalledWith('https://example.com/cover.jpg');
		expect(getMetadataForFile('/books/alpha.m4b')).toEqual(
			expect.objectContaining({
				cover_art: [9, 9, 9],
			}),
		);
	});

	it('preserves existing cover art when replace toggle is disabled', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({ input: fakeInput(), metadata });
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<App />
			</AppRuntimeProvider>
		));
		await importAndSelectAll(runtime);
		await userEvent.click(document.getElementById('metadata-lookup-btn') as HTMLElement);
		const apply = await waitFor(() => {
			const button = document.querySelector<HTMLButtonElement>(
				"#metadata-lookup-results button[data-index='0']",
			);
			if (!button) throw new Error('Expected an apply button');
			return button;
		});
		await userEvent.click(apply);
		await waitFor(() => {
			expect(getStatusText()).toContain('Metadata applied.');
		});
		expect(metadata.loadCoverArtFromUrl).toHaveBeenCalledWith('https://example.com/cover.jpg');
		expect(getMetadataForFile('/books/alpha.m4b')).toEqual(
			expect.objectContaining({
				cover_art: [1, 1, 1],
			}),
		);
	});

	it('does not mutate metadata when skipping queue item', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({ input: fakeInput(), metadata });
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<App />
			</AppRuntimeProvider>
		));
		await importAndSelectAll(runtime);
		const before = getMetadataForFile('/books/alpha.m4b');
		await userEvent.click(document.getElementById('metadata-lookup-btn') as HTMLElement);
		await waitFor(() => {
			expect(document.getElementById('metadata-lookup-skip-btn')).toBeTruthy();
		});
		await userEvent.click(document.getElementById('metadata-lookup-skip-btn') as HTMLElement);
		await waitFor(() => {
			expect(getStatusText()).toContain('Skipped.');
		});
		expect(getContextText()).toContain('beta.m4b');
		expect(getMetadataForFile('/books/alpha.m4b')).toEqual(before);
	});

	it('shows manual-entry CTA when search returns no results and focuses metadata title', async () => {
		const metadata = fakeMetadata({
			searchOnlineMetadata: vi.fn(async () => ({ results: [], diagnostics: [] })),
		});
		runtime = createTestAppRuntime({ input: fakeInput(), metadata });
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<App />
			</AppRuntimeProvider>
		));
		await importAndSelectAll(runtime);
		await userEvent.click(document.getElementById('metadata-lookup-btn') as HTMLElement);
		await waitFor(() => {
			expect(document.body.textContent ?? '').toContain(
				'Older CD-era or rare audiobook editions may not be indexed.',
			);
		});
		await userEvent.click(
			document.getElementById('metadata-lookup-manual-entry-btn') as HTMLElement,
		);
		await waitFor(() => {
			expect(document.getElementById('metadata-lookup-modal')?.classList.contains('open')).toBe(
				false,
			);
			expect((document.activeElement as HTMLElement | null)?.id).toBe('meta-title');
		});
	});

	it('keeps backend failure distinct from no-result state', async () => {
		const metadata = fakeMetadata({
			searchOnlineMetadata: vi.fn(async () => {
				throw new Error('all sources failed');
			}),
		});
		runtime = createTestAppRuntime({ input: fakeInput(), metadata });
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<App />
			</AppRuntimeProvider>
		));
		await importAndSelectAll(runtime);
		await userEvent.click(document.getElementById('metadata-lookup-btn') as HTMLElement);
		await waitFor(() => {
			expect(getStatusText()).toBe('Search failed. Check your query and try again.');
		});
		expect(document.body.textContent ?? '').not.toContain(
			'Older CD-era or rare audiobook editions may not be indexed.',
		);
		expect(document.getElementById('metadata-lookup-manual-entry-btn')).toBeNull();
		expect(document.querySelector("#metadata-lookup-results button[data-index='0']")).toBeNull();
	});
});
