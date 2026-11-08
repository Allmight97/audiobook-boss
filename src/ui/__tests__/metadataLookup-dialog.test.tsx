import { screen, waitFor, within } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { LookupSnapshot, SessionIntent } from '../../lib/generated/tauri';
import { audioFile, createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { lookupResult } from '../../test/fixtures/lookup';
import { renderApp } from '../../test/fixtures/renderApp';

function lookupDialog(): HTMLElement {
	return screen.getByRole('dialog', { name: 'Find Metadata Online', hidden: true });
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
		await userEvent.click(screen.getByRole('button', { name: 'Add audio files' }));
		await waitFor(() => expect(app.input.view().files.length).toBe(2));
		app.input.selectAll();
		await userEvent.click(screen.getByRole('button', { name: 'Find Metadata' }));
		await waitFor(() => expect(lookupDialog()).toHaveAttribute('data-state', 'open'));
	}

	it('sends the cover choice and the chosen result, then shows the next queued title', async () => {
		await openLookupOverTwoTitles({
			lookupApply: queued(1, {
				status: { kind: 'found', count: 1, partial: false, after: 'applied' },
			}),
		});
		const dialog = lookupDialog();
		expect(within(dialog).getByText(/1 of 2 • Alpha/)).toBeInTheDocument();

		await userEvent.click(within(dialog).getByRole('checkbox', { name: 'Replace cover art' }));
		await userEvent.click(within(dialog).getByRole('button', { name: 'Use Metadata' }));

		expect(await within(dialog).findByText(/Metadata applied\./)).toBeInTheDocument();
		expect(within(dialog).getByText(/2 of 2 • Beta/)).toBeInTheDocument();
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

		const dialog = lookupDialog();
		await userEvent.click(within(dialog).getByRole('button', { name: 'Skip' }));

		expect(await within(dialog).findByText(/Skipped\./)).toBeInTheDocument();
		expect(within(dialog).getByText(/Beta/)).toBeInTheDocument();
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
		const dialog = lookupDialog();
		expect(
			await within(dialog).findByText(/Older CD-era or rare audiobook editions/),
		).toBeInTheDocument();

		await userEvent.click(within(dialog).getByRole('button', { name: 'Use Manual Entry' }));

		await waitFor(() => {
			expect(dialog).toHaveAttribute('data-state', 'closed');
			expect(screen.getByLabelText('Book Title')).toHaveFocus();
		});
	});

	it('says no online matches were found for an empty search', async () => {
		await openLookupOverTwoTitles({
			lookupOpen: queued(0, {
				results: [],
				status: { kind: 'found', count: 0, partial: false, after: null },
			}),
		});

		expect(await within(lookupDialog()).findByText('No online matches found.')).toBeInTheDocument();
	});

	it('shows a failed search as a failure, not as no matches', async () => {
		await openLookupOverTwoTitles({
			lookupOpen: queued(0, {
				results: [],
				hasSearched: false,
				status: { kind: 'searchFailed', after: null },
			}),
		});

		const dialog = lookupDialog();
		expect(
			await within(dialog).findByText('Search failed. Check your query and try again.'),
		).toBeInTheDocument();
		expect(within(dialog).queryByText(/Older CD-era or rare audiobook editions/)).toBeNull();
		expect(within(dialog).queryByRole('button', { name: 'Use Manual Entry' })).toBeNull();
		expect(within(dialog).queryByRole('button', { name: 'Use Metadata' })).toBeNull();
	});
});
