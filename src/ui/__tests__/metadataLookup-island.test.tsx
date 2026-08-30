import { cleanup, render, waitFor } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { lookupViewAtom } from '../../app/metadataLookup';
import { metadataLookupState, snapshotMetadataLookupState } from '../../app/metadataLookup/state';
import { clearMetadataLookupCoverPreviewCache } from '../../app/metadataLookup/coverPreview';
import { AppRuntimeProvider } from '../../app/runtime/RuntimeProvider';
import { createTestAppRuntime } from '../../app/runtime/harness';
import type { AppRuntime } from '../../app/runtime';
import type { MetadataCapability } from '../../lib/tauri/capabilities/metadata';
import { MetadataLookupView } from '../metadataLookup/MetadataLookupView';

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
		searchOnlineMetadata: vi.fn(async () => ({ results: [], diagnostics: [] })),
		...overrides,
	};
}

describe('MetadataLookup cover preview', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
		clearMetadataLookupCoverPreviewCache();
	});

	it('eagerly loads cover previews through the backend without exposing provider URLs', async () => {
		const metadata = fakeMetadata();
		runtime = createTestAppRuntime({ metadata });
		render(() => (
			<AppRuntimeProvider runtime={runtime!}>
				<button id="metadata-lookup-btn" type="button">
					Open
				</button>
				<MetadataLookupView />
			</AppRuntimeProvider>
		));
		metadataLookupState.isOpen = true;
		metadataLookupState.hasSearched = true;
		metadataLookupState.results = [
			{
				source: 'audnexus',
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
				source: 'openlibrary',
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
		];
		runtime.registry.set(lookupViewAtom, snapshotMetadataLookupState());

		await waitFor(() => {
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
