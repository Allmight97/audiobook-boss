import { describe, expect, it } from 'vitest';

import { pathBasename } from './basename';

describe('pathBasename', () => {
	it('returns the final path segment while ignoring trailing separators', () => {
		expect(pathBasename('foo/bar/', { fallback: 'path' })).toBe('bar');
		expect(pathBasename('foo\\bar\\', { fallback: 'path' })).toBe('bar');
	});

	it('returns an empty string for empty paths under either fallback', () => {
		expect(pathBasename('', { fallback: 'path' })).toBe('');
		expect(pathBasename('', { fallback: 'empty' })).toBe('');
	});

	it('uses the fallback for separator-only paths', () => {
		expect(pathBasename('/', { fallback: 'path' })).toBe('/');
		expect(pathBasename('/', { fallback: 'empty' })).toBe('');
	});
});
