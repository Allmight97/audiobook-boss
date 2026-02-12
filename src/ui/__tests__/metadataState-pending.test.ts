import { beforeEach, describe, expect, it } from 'vitest';
import {
	clearMetadataState,
	clearPendingMetadataForFile,
	getPendingMetadataEntries,
	hasPendingMetadataChanges,
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
});
