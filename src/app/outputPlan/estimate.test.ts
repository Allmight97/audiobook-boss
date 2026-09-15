import { describe, expect, it } from 'vitest';
import { estimateEncodedSizeBytes, formatEstimatedSizeText } from './estimate';

describe('estimateEncodedSizeBytes', () => {
	it('returns zero for a missing or non-positive duration', () => {
		expect(estimateEncodedSizeBytes(0, { bitrateKbps: 64 })).toBe(0);
		expect(estimateEncodedSizeBytes(-1, { bitrateKbps: 64 })).toBe(0);
	});

	it('uses total bitrate with 1.03 overhead', () => {
		// 100s * 64 kbps * 1000 / 8 = 800_000, then *1.03 overhead.
		expect(estimateEncodedSizeBytes(100, { bitrateKbps: 64 })).toBe(824000);
	});
});

describe('formatEstimatedSizeText', () => {
	it('keeps the empty-session placeholder when no files are present', () => {
		expect(formatEstimatedSizeText(false, 3600, { bitrateKbps: 64 })).toBe('~ --- MB');
	});

	it('formats a derived byte estimate for a populated session', () => {
		const text = formatEstimatedSizeText(true, 100, { bitrateKbps: 64 });
		expect(text.startsWith('~ ')).toBe(true);
		expect(text).not.toBe('~ --- MB');
	});
});
