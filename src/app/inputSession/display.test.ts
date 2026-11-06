import { describe, expect, it } from 'vitest';
import { audioFile } from '../../test/fixtures/fakeEngine';
import { formatFileDetails } from './display';

describe('file details', () => {
	it('shows an invalid file as an error', () => {
		const file = audioFile('/books/broken.m4b', {
			isValid: false,
			error: 'Broken audio source',
		});

		expect(formatFileDetails(file)).toBe('Error: Broken audio source');
	});

	it('shows an exported title whose download was removed without an error prefix', () => {
		const file = audioFile('/books/done.m4b', {
			isValid: false,
			sourceRemoved: true,
			error:
				'Exported. Its download was removed to save space; acquire it again to export it again.',
		});

		expect(formatFileDetails(file)).toBe(
			'Exported. Its download was removed to save space; acquire it again to export it again.',
		);
	});
});
