import { describe, expect, it } from 'vitest';
import type { LookupStatus, SessionLookup } from '../../types/session';
import { toLookupState } from './state';

function lookup(overrides: Partial<SessionLookup> = {}): SessionLookup {
	return {
		revision: 1,
		open: true,
		titleQuery: 'Dune',
		authorQuery: 'Herbert',
		source: 'auto',
		applyMode: 'current',
		replaceCover: false,
		status: null,
		queuePosition: null,
		results: [],
		isQueueMode: false,
		hasSearched: false,
		...overrides,
	};
}

describe('lookup wording', () => {
	it.each<[LookupStatus | null, string, 'error' | 'success' | 'info']>([
		[null, '', 'info'],
		[{ kind: 'noValidTitle' }, 'Select a valid file to search metadata.', 'error'],
		[{ kind: 'searching' }, 'Searching metadata sources…', 'info'],
		[{ kind: 'found', count: 3, partial: false, after: null }, 'Found 3 results.', 'success'],
		[
			{ kind: 'found', count: 1, partial: true, after: null },
			'Found 1 results. Some lookup data was unavailable; showing available results.',
			'info',
		],
		[
			{ kind: 'found', count: 2, partial: false, after: 'applied' },
			'Metadata applied. Found 2 results.',
			'success',
		],
		[
			{ kind: 'found', count: 2, partial: false, after: 'appliedWithoutCover' },
			'Metadata applied, but cover art failed to load. Found 2 results.',
			'error',
		],
		[
			{ kind: 'searchFailed', after: 'skipped' },
			'Skipped. Search failed. Check your query and try again.',
			'error',
		],
		[{ kind: 'applied', coverFailed: false }, 'Metadata applied to form.', 'success'],
		[
			{ kind: 'applied', coverFailed: true },
			'Metadata applied to form, but cover art failed to load.',
			'error',
		],
		[{ kind: 'queueComplete', coverFailed: false }, 'Queue complete.', 'success'],
	])('words %j', (status, message, variant) => {
		const state = toLookupState(lookup({ status }), {});
		expect(state.statusMessage).toBe(message);
		expect(state.statusVariant).toBe(variant);
	});

	it('names the queued title and shows query text typed before the engine confirms it', () => {
		const state = toLookupState(
			lookup({ queuePosition: { index: 1, total: 3, path: '/books/second.m4b' } }),
			{ titleQuery: 'Dune Mess' },
		);

		expect(state.queueContext).toBe('2 of 3 • second.m4b');
		expect(state.titleQuery).toBe('Dune Mess');
		expect(state.authorQuery).toBe('Herbert');
		expect(toLookupState(lookup(), {}).queueContext).toBe('No files selected.');
	});
});
