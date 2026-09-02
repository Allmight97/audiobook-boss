import { cleanup, render, waitFor } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { FileListInfo } from '../../types/audio';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
import type { InputCapability } from '../../lib/tauri/capabilities/input';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import { MetadataLookupView } from '../metadataLookup/MetadataLookupView';

function analyzedFile(path: string): FileListInfo['files'][number] {
	return {
		path,
		isValid: true,
		duration: 1,
		size: 1000,
		format: 'm4b',
		tagTitle: 'Private Cover',
		inputId: path,
	};
}

function fakeInput(): InputCapability {
	return {
		openFiles: vi.fn(async () => ['/books/alpha.m4b']),
		openDirectory: vi.fn(async () => null),
		discoverAudioImportPaths: vi.fn(async (paths) => [...paths]),
		analyzeAudioFiles: vi.fn(async (paths: ReadonlyArray<string>) => ({
			files: paths.map((path) => analyzedFile(path)),
			selectedDecoders: paths.map(() => null),
			totalDuration: paths.length,
			totalSize: paths.length * 1000,
			validCount: paths.length,
			invalidCount: 0,
		})),
		getSupportedAudioImportMetadata: vi.fn(async () => ({
			formats: [{ extension: 'm4b', label: 'M4B' }],
			extensions: ['m4b'],
			formatsText: 'M4B',
			supportText: 'Supports M4B audio files',
		})),
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
		readAudioMetadata: vi.fn(async () => ({})),
		validateMetadataIntentPatch: vi.fn(async (patch) => ({
			isValid: true,
			metadataPatch: patch,
			fieldErrors: [],
		})),
		saveMetadataBatch: vi.fn(async () => ({
			results: [],
			summary: { succeeded: 0, failed: 0, cancelled: 0, skipped: 0, total: 0 },
		})),
		openFile: vi.fn(async () => null),
		loadCoverArtFile: vi.fn(async () => []),
		loadCoverArtFromUrl: vi.fn(async () => [0xff, 0xd8, 0xff]),
		searchOnlineMetadata: vi.fn(async () => ({
			results: [
				{
					source: 'audnexus' as const,
					sourceId: 'audnexus:private',
					title: 'Private Cover',
					authors: ['Author One'],
					narrators: ['Narrator One'],
					description: 'Description',
					publishedDate: '2020-07',
					durationSeconds: 3600,
					audibleOnly: false,
					coverUrl: 'https://covers.example.com/private-cover.jpg',
				},
				{
					source: 'openlibrary' as const,
					sourceId: 'openlibrary:loopback',
					title: 'Loopback Cover',
					authors: ['Author Two'],
					narrators: ['Narrator Two'],
					description: 'Description',
					publishedDate: '2021-08',
					durationSeconds: 7200,
					audibleOnly: false,
					coverUrl: 'https://covers.example.com/loopback-cover.jpg',
				},
			],
			diagnostics: [],
		})),
		...overrides,
	};
}

describe('MetadataLookup cover preview', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	it('eagerly loads cover previews through the backend without exposing provider URLs', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({ input: fakeInput(), metadata });
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<button id="metadata-lookup-btn" type="button">
					Open
				</button>
				<MetadataLookupView />
			</AppRuntimeProvider>
		));
		await runtime.input.importIntent({ type: 'importPaths', paths: ['/books/alpha.m4b'] });
		await runtime.input.selectAll();
		await runtime.lookup.run({ type: 'open' });

		await waitFor(() => {
			expect(metadata.searchOnlineMetadata).toHaveBeenCalled();
			expect(metadata.loadCoverArtFromUrl).toHaveBeenCalledWith(
				'https://covers.example.com/private-cover.jpg',
			);
			expect(metadata.loadCoverArtFromUrl).toHaveBeenCalledWith(
				'https://covers.example.com/loopback-cover.jpg',
			);
		});

		await waitFor(() => {
			expect(document.querySelector('[data-testid="metadata-lookup-cover-image"]')).toBeTruthy();
		});

		const image = document.querySelector(
			'[data-testid="metadata-lookup-cover-image"]',
		) as HTMLImageElement | null;
		expect(image?.src.startsWith('data:image/jpeg;base64,')).toBe(true);
		expect(image?.src).not.toContain('covers.example.com');
		expect(document.querySelector('[src*="169.254.169.254"]')).toBeNull();
		expect(document.querySelector('[src*="127.0.0.1"]')).toBeNull();
	});
});
