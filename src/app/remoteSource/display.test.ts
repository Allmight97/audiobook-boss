import { describe, expect, it } from 'vitest';
import { releaseProtocolLabel } from './display';

describe('remote source acquisition display', () => {
	it('uses Prowlarr protocol words on release tags', () => {
		expect(releaseProtocolLabel('torrent')).toBe('torrent');
		expect(releaseProtocolLabel('usenet')).toBe('nzb');
		expect(releaseProtocolLabel('unknown')).toBe('unknown');
	});
});
