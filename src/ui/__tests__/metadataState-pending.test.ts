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
import {
	applyMetadataIntentPatch,
	buildMetadataIntentPatchFromMetadata,
} from '../../types/metadataIntent';
import { buildMetadataDraftIntent } from '../metadataDraft';

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

	it('uses UI draft intent when no explicit pending patch is provided', () => {
		setMetadataForFile(
			'/a.mp3',
			{
				title: 'Book A',
				album_sort: 'Curated Sort',
				comment: 'Reader note',
			},
			{ markPending: true },
		);

		expect(getPendingMetadataIntentEntries()).toEqual([
			['/a.mp3', buildMetadataDraftIntent({ title: 'Book A' })],
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

	it('preserves explicit clear patch when merged metadata omits cleared key', () => {
		const merged = applyMetadataIntentPatch(
			{ title: 'Book A', artist: 'Author A' },
			{ title: { op: 'clear' } },
		);

		setMetadataForFile('/a.mp3', merged, {
			markPending: true,
			intentPatch: { title: { op: 'clear' } },
		});

		expect(merged).toEqual({ artist: 'Author A' });
		expect(getPendingMetadataIntentEntries()).toEqual([['/a.mp3', { title: { op: 'clear' } }]]);
	});
});
