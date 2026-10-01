import { cleanup, render, waitFor } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { OnlineMetadataResult } from '../../types/metadata';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';

import { createFakeEngine } from '../../test/fixtures/fakeEngine';
import { MetadataLookupView } from '../metadataLookup/MetadataLookupView';

function coverResult(source: 'audnexus' | 'openlibrary', name: string): OnlineMetadataResult {
	return {
		source,
		sourceId: `${source}:${name}`,
		title: `${name} Cover`,
		authors: ['Author'],
		narrators: ['Narrator'],
		description: 'Description',
		publishedDate: '2020-07',
		durationSeconds: 3600,
		audibleOnly: false,
		coverUrl: `https://covers.example.com/${name}-cover.jpg`,
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
		const engine = createFakeEngine();
		engine.lookupResults = [
			coverResult('audnexus', 'private'),
			coverResult('openlibrary', 'loopback'),
		];
		const loadCoverArtFromUrl = vi.fn(async () => [0xff, 0xd8, 0xff]);
		runtime = createAppRuntime({
			engine,
			metadata: { openFile: vi.fn(async () => null), loadCoverArtFromUrl },
		});
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
			expect(loadCoverArtFromUrl).toHaveBeenCalledWith(
				'https://covers.example.com/private-cover.jpg',
			);
			expect(loadCoverArtFromUrl).toHaveBeenCalledWith(
				'https://covers.example.com/loopback-cover.jpg',
			);
		});

		await waitFor(() => {
			const images = document.querySelectorAll<HTMLImageElement>(
				'[data-testid="metadata-lookup-cover-image"]',
			);
			expect(images).toHaveLength(2);
			for (const image of images)
				expect(image.src.startsWith('data:image/jpeg;base64,')).toBe(true);
		});
		for (const source of document.querySelectorAll('[src]')) {
			expect(source.getAttribute('src')).not.toContain('covers.example.com');
		}
	});
});
