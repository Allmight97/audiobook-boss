import { describe, expect, it } from 'vitest';
import { type AppErrorEnvelope, isCancellation, toUserMessage } from './appError';

function envelope(partial: Partial<AppErrorEnvelope>): AppErrorEnvelope {
	return {
		code: 'some_code',
		category: 'processing',
		message: 'something failed',
		...partial,
	};
}

describe('toUserMessage', () => {
	it('returns the normalized message for typed envelopes', () => {
		expect(toUserMessage(envelope({ message: 'boom' }))).toBe('boom');
	});

	it('falls back when the message is empty', () => {
		expect(toUserMessage(envelope({ message: '' }), { fallback: 'fallback' })).toBe('fallback');
	});

	it('collapses unknown-coded errors to the fallback when suppressUnknown is set', () => {
		expect(
			toUserMessage(new Error('internal detail'), {
				fallback: 'Friendly message',
				suppressUnknown: true,
			}),
		).toBe('Friendly message');
	});

	it('keeps the message for unknown errors when suppressUnknown is not set', () => {
		expect(toUserMessage(new Error('raw detail'), { fallback: 'Friendly message' })).toBe(
			'raw detail',
		);
	});
});

describe('isCancellation', () => {
	it('detects the typed cancellation category', () => {
		expect(isCancellation(envelope({ category: 'cancellation', message: 'done' }))).toBe(true);
	});

	it('reads only the category, not words in the message', () => {
		expect(
			isCancellation(envelope({ category: 'io', message: 'Could not write cancelled.m4b' })),
		).toBe(false);
		expect(isCancellation(new Error('Processing was cancelled.'))).toBe(false);
	});

	it('returns false for unrelated errors', () => {
		expect(isCancellation(envelope({ category: 'processing', message: 'encode failed' }))).toBe(
			false,
		);
		expect(isCancellation(new Error('disk full'))).toBe(false);
	});
});
