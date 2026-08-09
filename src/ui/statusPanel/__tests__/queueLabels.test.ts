import { describe, expect, it } from 'vitest';
import { buildQueueLabels, formatEtaRemaining } from '../formatting';

describe('formatEtaRemaining', () => {
	it('formats sub-hour and hour-plus remainders', () => {
		expect(formatEtaRemaining(242)).toBe('04:02 left');
		expect(formatEtaRemaining(3722)).toBe('1:02:02 left');
	});

	it('rounds seconds and clamps negative estimates', () => {
		expect(formatEtaRemaining(59.6)).toBe('01:00 left');
		expect(formatEtaRemaining(-5)).toBe('00:00 left');
	});
});

describe('buildQueueLabels', () => {
	it('extends parent segments until labels are unique', () => {
		const labels = buildQueueLabels(['/d/e/c.mp3', '/f/e/c.mp3', '/x/y/c.mp3']);

		expect(labels).toEqual(['d/e/c.mp3', 'f/e/c.mp3', 'x/y/c.mp3']);
	});

	it('keeps basenames when already unique', () => {
		const labels = buildQueueLabels(['/books/alpha.m4b', '/books/beta.m4b']);

		expect(labels).toEqual(['alpha.m4b', 'beta.m4b']);
	});
});
