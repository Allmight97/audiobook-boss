import { describe, expect, it } from 'vitest';

import { isProcessingCancellationError, normalizeProcessingErrorMessage } from './errorHelpers';

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

	it('uses structured app error envelopes directly', () => {
		expect(
			normalizeProcessingErrorMessage({
				code: 'decoder_unavailable',
				category: 'toolchain',
				message: 'decoder unavailable',
				detail: 'ffmpeg missing',
			}),
		).toBe('decoder unavailable');
	});

	it('detects cancellation errors by category', () => {
		expect(
			isProcessingCancellationError({
				code: 'cancelled',
				category: 'cancellation',
				message: 'Processing was cancelled.',
			}),
		).toBe(true);
		expect(isProcessingCancellationError({ message: 'Processing was cancelled.' })).toBe(false);
	});

	it('returns fallback for null or undefined errors', () => {
		expect(normalizeProcessingErrorMessage(null)).toBe('Unknown error');
		expect(normalizeProcessingErrorMessage(undefined, 'fallback')).toBe('fallback');
	});
});
