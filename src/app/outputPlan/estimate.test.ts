import { describe, expect, it } from 'vitest';
import { estimateEncodedSizeBytes } from './estimate';

describe('estimateEncodedSizeBytes', () => {
	it('returns zero for a missing or non-positive duration', () => {
		expect(estimateEncodedSizeBytes(0, 64)).toBe(0);
		expect(estimateEncodedSizeBytes(-1, 64)).toBe(0);
	});

	it('uses total bitrate with 1.03 overhead', () => {
		// 100s * 64 kbps * 1000 / 8 = 800_000, then *1.03 overhead.
		expect(estimateEncodedSizeBytes(100, 64)).toBe(824000);
	});
});
