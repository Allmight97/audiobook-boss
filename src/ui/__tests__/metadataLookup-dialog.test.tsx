import { waitFor } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { LookupSnapshot, SessionIntent } from '../../lib/generated/tauri';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { lookupResult } from '../../test/fixtures/lookup';
import { renderApp } from '../../test/fixtures/renderApp';

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

function queued(index: number, patch: Partial<LookupSnapshot> = {}): Partial<LookupSnapshot> {
	return {
		open: true,
		isQueueMode: true,
		applyMode: 'queue',
		queuePosition: {
			index,
			total: 2,
			path: index === 0 ? '/books/alpha.m4b' : '/books/beta.m4b',
			label: index === 0 ? 'Alpha' : 'Beta',
		},
		results: [lookupResult()],
		hasSearched: true,
		status: { kind: 'found', count: 1, partial: false, after: null },
		...patch,
	};
}

describe('metadata lookup dialog', () => {
	let engine: FakeEngine;

	async function openLookupOverTwoTitles(
		answers: Partial<Record<SessionIntent['kind'], Partial<LookupSnapshot>>> = {},
	) {
		engine = createFakeEngine();
		engine.analyze = (paths) =>
			paths.map((path) =>
				audioFile(path, { status: 'ready', label: path.includes('beta') ? 'Beta' : 'Alpha' }),
			);
		const lookupAfter: Partial<Record<SessionIntent['kind'], Partial<LookupSnapshot>>> = {
			lookupOpen: queued(0),
			...answers,
		};
		engine.respond = (intent) => {
			const lookup = lookupAfter[intent.kind];
			if (!lookup) return undefined;
			engine.change((state) => {
				state.lookup = { ...state.lookup, ...lookup };
			});
			return { kind: 'applied' };
		};
		const { runtime: app } = await renderApp({
			engine,
			input: { openFiles: vi.fn(async () => ['/books/alpha.m4b', '/books/beta.m4b']) },
		});
		await userEvent.click(
			document.querySelector<HTMLButtonElement>('[aria-label="Add audio files"]') as HTMLElement,
		);
		await waitFor(() => expect(app.input.view().files.length).toBe(2));
		app.input.selectAll();
		await userEvent.click(document.getElementById('metadata-lookup-btn') as HTMLElement);
		await waitFor(() =>
			expect(document.getElementById('metadata-lookup-modal')?.classList.contains('open')).toBe(
				true,
			),
		);
	}

	it('sends the cover choice and the chosen result, then shows the next queued title', async () => {
		await openLookupOverTwoTitles({
			lookupApply: queued(1, {
				status: { kind: 'found', count: 1, partial: false, after: 'applied' },
			}),
		});
		expect(getContextText()).toContain('1 of 2 • Alpha');

		await userEvent.click(document.getElementById('metadata-lookup-cover-toggle') as HTMLElement);
		await userEvent.click(await waitFor(applyButton));

		await waitFor(() => expect(getStatusText()).toContain('Metadata applied.'));
		expect(getContextText()).toContain('2 of 2 • Beta');
		const sent = engine.sessionIntents.map((intent) => intent.kind);
		expect(sent.indexOf('lookupSetReplaceCover')).toBeLessThan(sent.indexOf('lookupApply'));
		expect(engine.sessionIntents).toContainEqual({ kind: 'lookupSetReplaceCover', replace: true });
		expect(engine.sessionIntents).toContainEqual({
			kind: 'lookupApply',
			index: 0,
			revision: expect.any(Number),
		});
	});

	it('sends a skip and shows the next queued title', async () => {
		await openLookupOverTwoTitles({
			lookupSkip: queued(1, {
				status: { kind: 'found', count: 1, partial: false, after: 'skipped' },
			}),
		});

		await userEvent.click(document.getElementById('metadata-lookup-skip-btn') as HTMLElement);

		await waitFor(() => expect(getStatusText()).toContain('Skipped.'));
		expect(getContextText()).toContain('Beta');
		expect(engine.sessionIntents).toContainEqual({ kind: 'lookupSkip' });
	});

	it('offers manual entry when a search finds nothing, and focuses the title field', async () => {
		await openLookupOverTwoTitles({
			lookupOpen: queued(0, {
				results: [],
				status: { kind: 'found', count: 0, partial: false, after: null },
			}),
			lookupClose: { open: false },
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

	it('says no online matches were found for an empty search', async () => {
		await openLookupOverTwoTitles({
			lookupOpen: queued(0, {
				results: [],
				status: { kind: 'found', count: 0, partial: false, after: null },
			}),
		});

		await waitFor(() =>
			expect(document.body.textContent ?? '').toContain('No online matches found.'),
		);
	});

	it('shows a failed search as a failure, not as no matches', async () => {
		await openLookupOverTwoTitles({
			lookupOpen: queued(0, {
				results: [],
				hasSearched: false,
				status: { kind: 'searchFailed', after: null },
			}),
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
