import { beforeEach, describe, expect, it } from 'vitest';
import { clearMetadataState, isUsableMetadataCache, setMetadataForFile } from '../metadataState';

describe('metadataState cache usability', () => {
	beforeEach(() => {
		clearMetadataState();
	});

	it('treats undefined and empty objects as unusable cache entries', () => {
		expect(isUsableMetadataCache(undefined)).toBe(false);
		expect(isUsableMetadataCache({})).toBe(false);
	});

	it('treats cover-only cache entries as unusable', () => {
		expect(isUsableMetadataCache({ cover_art: [1, 2, 3] })).toBe(false);
	});

	it('treats metadata with substantive fields as usable', () => {
		expect(isUsableMetadataCache({ title: 'Book Title' })).toBe(true);
		expect(isUsableMetadataCache({ title: 'Book Title', cover_art: [1, 2, 3] })).toBe(true);
	});

	it('stores and returns usable metadata through getMetadataForFile', () => {
		setMetadataForFile('/books/alpha.m4b', { title: 'Alpha' });
		expect(isUsableMetadataCache({ title: 'Alpha' })).toBe(true);
	});
});
