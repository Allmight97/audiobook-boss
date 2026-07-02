import { beforeEach, describe, expect, it } from 'vitest';
import {
	cacheMetadataForFile,
	clearMetadataSession,
	clearPendingMetadataForFile,
	getMetadataForFile,
	getPendingMetadataIntentEntries,
	removeMetadataForFile,
	stageMetadataIntentPatch,
} from '../state';
import { buildMetadataIntentPatchFromMetadata } from '../../../types/metadataIntent';

describe('metadataSession pending draft tracking', () => {
	beforeEach(() => {
		clearMetadataSession();
	});

	it('does not mark pending on plain cache writes', () => {
		cacheMetadataForFile('/a.mp3', { title: 'Book A' });
		expect(getPendingMetadataIntentEntries()).toEqual([]);
	});

	it('tracks pending entries when intent is staged', () => {
		expect(
			stageMetadataIntentPatch('/a.mp3', buildMetadataIntentPatchFromMetadata({ title: 'Book A' })),
		).toBe('staged');
		expect(getMetadataForFile('/a.mp3')).toEqual({ title: 'Book A' });
		expect(getPendingMetadataIntentEntries()).toEqual([
			['/a.mp3', { title: { op: 'set', value: 'Book A' } }],
		]);
	});

	it('clears pending state after successful save', () => {
		stageMetadataIntentPatch('/a.mp3', { title: { op: 'set', value: 'Book A' } });
		clearPendingMetadataForFile('/a.mp3');
		expect(getPendingMetadataIntentEntries()).toEqual([]);
	});

	it('removing file metadata also removes pending marker', () => {
		stageMetadataIntentPatch('/a.mp3', { title: { op: 'set', value: 'Book A' } });
		removeMetadataForFile('/a.mp3');
		expect(getPendingMetadataIntentEntries()).toEqual([]);
		expect(getMetadataForFile('/a.mp3')).toBeUndefined();
	});

	it('treats null and undefined as equivalent when deciding unchanged', () => {
		// Cache holds explicit null (wire shape); staging a clear produces an
		// equal-modulo-nullish merge and must not create pending truth.
		cacheMetadataForFile('/a.mp3', { title: null } as never);
		expect(stageMetadataIntentPatch('/a.mp3', { title: { op: 'clear' } })).toBe('unchanged');
		expect(getPendingMetadataIntentEntries()).toEqual([]);

		// A real value difference still stages.
		cacheMetadataForFile('/b.mp3', { series_part: '1.0' });
		expect(stageMetadataIntentPatch('/b.mp3', { series_part: { op: 'clear' } })).toBe('staged');
	});

	it('tracks explicit clear intent and drops the cleared key from the cache', () => {
		cacheMetadataForFile('/a.mp3', { title: 'Book A', artist: 'Author A' });
		expect(stageMetadataIntentPatch('/a.mp3', { title: { op: 'clear' } })).toBe('staged');

		expect(getMetadataForFile('/a.mp3')).toEqual({ artist: 'Author A' });
		expect(getPendingMetadataIntentEntries()).toEqual([['/a.mp3', { title: { op: 'clear' } }]]);

		clearPendingMetadataForFile('/a.mp3');
		expect(getPendingMetadataIntentEntries()).toEqual([]);
	});

	it('merges successive staged patches into one stored intent', () => {
		stageMetadataIntentPatch('/a.mp3', { title: { op: 'set', value: 'Book A' } });
		stageMetadataIntentPatch('/a.mp3', { artist: { op: 'set', value: 'Author A' } });
		expect(getPendingMetadataIntentEntries()).toEqual([
			[
				'/a.mp3',
				{
					title: { op: 'set', value: 'Book A' },
					artist: { op: 'set', value: 'Author A' },
				},
			],
		]);
	});
});
