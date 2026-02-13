import { describe, expect, it } from 'vitest';

import { normalizeProcessingErrorMessage } from './errorHelpers';

describe('normalizeProcessingErrorMessage', () => {
	it('uses message from Error instances', () => {
		expect(normalizeProcessingErrorMessage(new Error('boom'))).toBe('boom');
	});

	it('returns string errors directly', () => {
		expect(normalizeProcessingErrorMessage('plain failure')).toBe('plain failure');
	});

	it('uses string message from object-shaped errors', () => {
		expect(normalizeProcessingErrorMessage({ message: 'object failure' })).toBe('object failure');
	});

	it('returns fallback for null or undefined errors', () => {
		expect(normalizeProcessingErrorMessage(null)).toBe('Unknown error');
		expect(normalizeProcessingErrorMessage(undefined, 'fallback')).toBe('fallback');
	});
});
