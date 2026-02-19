import { beforeEach, describe, expect, it } from 'vitest';
import {
	clearMetadataState,
	clearPendingMetadataForFile,
	hasMeaningfulMetadata,
	getPendingMetadataEntries,
	hasPendingMetadataChanges,
	metadataEqualsNullish,
	removeMetadataForFile,
	setMetadataForFile,
} from '../metadataState';

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
		setMetadataForFile('/a.mp3', { title: 'Book A' }, { markPending: true });
		expect(hasPendingMetadataChanges()).toBe(true);
		expect(getPendingMetadataEntries()).toEqual([['/a.mp3', { title: 'Book A' }]]);
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

	it('only marks metadata as meaningful when values carry write intent', () => {
		expect(
			hasMeaningfulMetadata({ title: undefined, artist: null } as unknown as Parameters<
				typeof hasMeaningfulMetadata
			>[0]),
		).toBe(false);
		expect(hasMeaningfulMetadata({ title: '   ' })).toBe(false);
		expect(hasMeaningfulMetadata({ title: 'Book A' })).toBe(true);
		expect(hasMeaningfulMetadata({ cover_art: [] })).toBe(true);
	});
});
