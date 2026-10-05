import { describe, expect, it } from 'vitest';
import { coverSrc } from './coverSrc';

/** The request path the engine parses: decoded once as a whole. */
function requestOf(address: string): string {
	return decodeURIComponent(address.replace('abb-cover://localhost/', ''));
}

describe('coverSrc', () => {
	it('names each cover the way the engine parses it, its value encoded once more', () => {
		const url = 'https://covers.test/a b/c.jpg?x=1';
		expect(requestOf(coverSrc({ kind: 'remote', url, size: 'small' }))).toBe(
			`remote/small/0/${encodeURIComponent(url)}`,
		);
		expect(requestOf(coverSrc({ kind: 'audio', path: '/books/a/b.m4b', revision: 3 }))).toBe(
			`audio/small/3/${encodeURIComponent('/books/a/b.m4b')}`,
		);
		expect(requestOf(coverSrc({ kind: 'session', revision: 7 }))).toBe('session/full/7/');
		expect(requestOf(coverSrc({ kind: 'preview', runId: 'run-1' }))).toBe('preview/small/0/run-1');
	});

	it('gives a changed cover a new address', () => {
		expect(coverSrc({ kind: 'session', revision: 1 })).not.toBe(
			coverSrc({ kind: 'session', revision: 2 }),
		);
	});
});
