import { describe, expect, it } from 'vitest';
import { buildQueueLabels } from '../formatting';

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
