import { beforeEach, describe, expect, it } from 'vitest';
import {
	cacheMetadataForFile,
	clearMetadataSession,
	getMetadataForFile,
	isUsableMetadataCache,
} from '../state';

describe('metadataSession cache usability', () => {
	beforeEach(() => {
		clearMetadataSession();
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
		cacheMetadataForFile('/books/alpha.m4b', { title: 'Alpha' });
		expect(isUsableMetadataCache(getMetadataForFile('/books/alpha.m4b'))).toBe(true);
	});
});
