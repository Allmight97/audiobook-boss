import { cleanup, render, waitFor } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { SupportedAudioImportMetadata } from '../../types/audio';
import type { OnlineMetadataResult } from '../../types/metadata';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';

import type { InputCapability } from '../../lib/tauri/capabilities/input';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { App } from '../App';

// The lookup's rules (queue order, which title a result applies to, what a
// replaced cover stages) are proven in the engine. These tests cover what the
// dialog does with the engine's answers and which intents it sends.

const support: SupportedAudioImportMetadata = {
	formats: [{ extension: 'm4b', label: 'M4B' }],
	extensions: ['m4b'],
	formatsText: 'M4B',
	supportText: 'Supports M4B audio files',
};

function lookupResult(): OnlineMetadataResult {
	return {
		source: 'audnexus',
		sourceId: 'audnexus:1',
		title: 'Lookup Title',
		authors: ['Author One'],
		narrators: ['Narrator One'],
		description: 'Description',
		publishedDate: '2020-07',
		durationSeconds: 3600,
		audibleOnly: false,
		coverUrl: 'https://example.com/cover.jpg',
	};
}

function fakeInput(): InputCapability {
	return {
		openFiles: vi.fn(async () => ['/books/alpha.m4b', '/books/beta.m4b']),
		openDirectory: vi.fn(async () => null),
		getSupportedAudioImportMetadata: vi.fn(async () => support),
		readAudioCoverThumbnail: vi.fn(async () => null),
		listenDragDrop: vi.fn(async () => () => undefined),
		listenDragEnter: vi.fn(async () => () => undefined),
		listenDragLeave: vi.fn(async () => () => undefined),
		listenOpenedAudioFiles: vi.fn(async () => () => undefined),
	};
}

function getStatusText(): string {
	return (document.getElementById('metadata-lookup-status') as HTMLElement).textContent ?? '';
}

function getContextText(): string {
	return (document.getElementById('metadata-lookup-context') as HTMLElement).textContent ?? '';
}

function applyButton(): HTMLButtonElement {
	const button = document.querySelector<HTMLButtonElement>(
		"#metadata-lookup-results button[data-index='0']",
	);
	if (!button) throw new Error('Expected an apply button');
	return button;
}

describe('metadata lookup dialog', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		cleanup();
		runtime?.dispose();
		runtime = undefined;
	});

	/** Renders the app with two titles imported and selected, and the lookup open. */
	async function openLookupOverTwoTitles(prepare: (engine: FakeEngine) => void = () => {}) {
		engine = createFakeEngine();
		engine.analyze = (paths) =>
			paths.map((path) => audioFile(path, { tagTitle: path.includes('beta') ? 'Beta' : 'Alpha' }));
		engine.lookupResults = [lookupResult()];
		prepare(engine);
		const app = createAppRuntime({ input: fakeInput(), engine });
		runtime = app;
		render(() => (
			<AppRuntimeProvider runtime={app}>
				<App />
			</AppRuntimeProvider>
		));
		await userEvent.click(
			document.querySelector<HTMLButtonElement>('[aria-label="Add audio files"]') as HTMLElement,
		);
		await waitFor(() => expect(app.input.view().files.length).toBe(2));
		await app.input.selectAll();
		await userEvent.click(document.getElementById('metadata-lookup-btn') as HTMLElement);
		await waitFor(() =>
			expect(document.getElementById('metadata-lookup-modal')?.classList.contains('open')).toBe(
				true,
			),
		);
	}

	it('sends the cover choice and the chosen result, then shows the next queued title', async () => {
		await openLookupOverTwoTitles();
		expect(getContextText()).toContain('1 of 2 • alpha.m4b');

		await userEvent.click(document.getElementById('metadata-lookup-cover-toggle') as HTMLElement);
		await userEvent.click(await waitFor(applyButton));

		await waitFor(() => expect(getStatusText()).toContain('Metadata applied.'));
		expect(getContextText()).toContain('2 of 2 • beta.m4b');
		const sent = engine.sessionIntents.map((intent) => intent.kind);
		expect(sent.indexOf('lookupSetReplaceCover')).toBeLessThan(sent.indexOf('lookupApply'));
		expect(engine.sessionIntents).toContainEqual({ kind: 'lookupSetReplaceCover', replace: true });
		expect(engine.sessionIntents).toContainEqual({ kind: 'lookupApply', index: 0 });
	});

	it('sends a skip and shows the next queued title', async () => {
		await openLookupOverTwoTitles();

		await userEvent.click(document.getElementById('metadata-lookup-skip-btn') as HTMLElement);

		await waitFor(() => expect(getStatusText()).toContain('Skipped.'));
		expect(getContextText()).toContain('beta.m4b');
		expect(engine.sessionIntents).toContainEqual({ kind: 'lookupSkip' });
		expect(engine.pendingEdits('/books/alpha.m4b')).toBeUndefined();
	});

	it('offers manual entry when a search finds nothing, and focuses the title field', async () => {
		await openLookupOverTwoTitles((engine) => {
			engine.lookupResults = [];
		});
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

	it('shows a failed search as a failure, not as no matches', async () => {
		await openLookupOverTwoTitles((engine) => {
			engine.searchFails = true;
		});

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
