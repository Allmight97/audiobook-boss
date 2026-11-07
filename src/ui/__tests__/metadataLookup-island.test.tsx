import { cleanup, render, waitFor } from '@solidjs/testing-library';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { OnlineMetadataResult } from '../../types/metadata';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';

import { coverSrc } from '../../lib/tauri/coverSrc';
import { createFakeEngine } from '../../test/fixtures/fakeEngine';
import { MetadataLookupView } from '../metadataLookup';

function coverResult(source: 'audnexus' | 'openlibrary', name: string): OnlineMetadataResult {
	return {
		source,
		title: `${name} Cover`,
		authors: ['Author'],
		narrators: ['Narrator'],
		description: 'Description',
		publishedDate: '2020-07',
		durationSeconds: 3600,
		audibleOnly: false,
		series: null,
		seriesPart: null,
		subseries: null,
		subseriesPart: null,
		cover: `remote/0/${encodeURIComponent(`https://covers.example.com/${name}-cover.jpg`)}`,
	};
}

describe('MetadataLookup cover preview', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	it('loads result covers through the engine, never from the provider host', async () => {
		const engine = createFakeEngine();
		engine.respond = (intent) => {
			if (intent.kind !== 'lookupOpen') return undefined;
			engine.change((state) => {
				state.lookup = {
					...state.lookup,
					open: true,
					hasSearched: true,
					results: [coverResult('audnexus', 'private'), coverResult('openlibrary', 'loopback')],
					status: { kind: 'found', count: 2, partial: false, after: null },
				};
			});
			return { kind: 'applied' };
		};
		runtime = await createAppRuntime({
			engine,
			metadata: { openFile: vi.fn(async () => null) },
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
		runtime.input.selectAll();
		await runtime.lookup.run({ type: 'open' });

		await waitFor(() => {
			const images = document.querySelectorAll<HTMLImageElement>(
				'[data-testid="metadata-lookup-cover-image"]',
			);
			expect([...images].map((image) => image.getAttribute('src'))).toEqual(
				[coverResult('audnexus', 'private'), coverResult('openlibrary', 'loopback')].map((result) =>
					coverSrc(result.cover!),
				),
			);
		});
		for (const source of document.querySelectorAll('[src]')) {
			expect(source.getAttribute('src')?.startsWith('abb-cover://')).toBe(true);
		}
	});
});
