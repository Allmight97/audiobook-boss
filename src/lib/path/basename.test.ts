import { describe, expect, it } from 'vitest';

import { pathBasename, pathSegments } from './basename';

describe('pathBasename', () => {
	it('returns the final path segment while ignoring trailing separators', () => {
		expect(pathBasename('foo/bar/')).toBe('bar');
		expect(pathBasename('foo\\bar\\')).toBe('bar');
	});

	it('returns an empty string for empty paths under either fallback', () => {
		expect(pathBasename('')).toBe('');
		expect(pathBasename('', { fallback: 'empty' })).toBe('');
	});

	it('uses the fallback for separator-only paths', () => {
		expect(pathBasename('/')).toBe('/');
		expect(pathBasename('/', { fallback: 'empty' })).toBe('');
	});
});

describe('pathSegments', () => {
	it('returns non-empty path segments', () => {
		expect(pathSegments('/foo//bar/')).toEqual(['foo', 'bar']);
	});
});
