import { beforeEach, describe, expect, it } from 'vitest';
import {
	clearMetadataState,
	clearPendingMetadataForFile,
	getPendingMetadataIntentEntries,
	getPendingMetadataEntries,
	hasPendingMetadataChanges,
	metadataEqualsNullish,
	removeMetadataForFile,
	setMetadataForFile,
} from '../metadataState';
import { buildMetadataIntentPatchFromMetadata } from '../../types/metadataIntent';

describe('metadataState pending draft tracking', () => {
	beforeEach(() => {
		clearMetadataState();
	});

	it('does not mark pending by default', () => {
		setMetadataForFile('/a.mp3', { title: 'Book A' });
		expect(hasPendingMetadataChanges()).toBe(false);
		expect(getPendingMetadataEntries()).toEqual([]);
	});

	it('tracks pending entries when explicitly marked', () => {
		setMetadataForFile(
			'/a.mp3',
			{ title: 'Book A' },
			{
				markPending: true,
				intentPatch: buildMetadataIntentPatchFromMetadata({ title: 'Book A' }),
			},
		);
		expect(hasPendingMetadataChanges()).toBe(true);
		expect(getPendingMetadataEntries()).toEqual([['/a.mp3', { title: 'Book A' }]]);
		expect(getPendingMetadataIntentEntries()).toEqual([
			['/a.mp3', { title: { op: 'set', value: 'Book A' } }],
		]);
	});

	it('clears pending state after successful save', () => {
		setMetadataForFile('/a.mp3', { title: 'Book A' }, { markPending: true });
		clearPendingMetadataForFile('/a.mp3');
		expect(hasPendingMetadataChanges()).toBe(false);
	});

	it('removing file metadata also removes pending marker', () => {
		setMetadataForFile('/a.mp3', { title: 'Book A' }, { markPending: true });
		removeMetadataForFile('/a.mp3');
		expect(hasPendingMetadataChanges()).toBe(false);
	});

	it('treats null and undefined as equivalent for metadata comparison', () => {
		expect(
			metadataEqualsNullish({ title: undefined }, { title: null } as unknown as Parameters<
				typeof metadataEqualsNullish
			>[1]),
		).toBe(true);
		expect(metadataEqualsNullish({ series_part: '1.0' }, { series_part: '1.0' })).toBe(true);
		expect(
			metadataEqualsNullish({ series_part: '1.0' }, { series_part: null } as unknown as Parameters<
				typeof metadataEqualsNullish
			>[1]),
		).toBe(false);
	});

	it('tracks explicit clear intent in pending patch entries', () => {
		setMetadataForFile(
			'/a.mp3',
			{ title: '' },
			{
				markPending: true,
				intentPatch: buildMetadataIntentPatchFromMetadata({ title: '' }),
			},
		);
		expect(getPendingMetadataIntentEntries()).toEqual([['/a.mp3', { title: { op: 'clear' } }]]);
		clearPendingMetadataForFile('/a.mp3');
		expect(getPendingMetadataIntentEntries()).toEqual([]);
	});
});
