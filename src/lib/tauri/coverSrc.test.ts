import { describe, expect, it } from 'vitest';
import { coverSrc } from './coverSrc';

describe('coverSrc', () => {
	it('hands the engine address to the abb-cover scheme as one encoded path', () => {
		const address = 'audio/3/%2Fbooks%2Fa%20b.m4b';
		const src = coverSrc(address);
		expect(src.startsWith('abb-cover://localhost/')).toBe(true);
		expect(decodeURIComponent(src.replace('abb-cover://localhost/', ''))).toBe(address);
	});

	it('gives a changed address a new source', () => {
		expect(coverSrc('session/1/')).not.toBe(coverSrc('session/2/'));
	});
});
