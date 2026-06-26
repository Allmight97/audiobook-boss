import { describe, expect, it, vi } from 'vitest';
import { type AppErrorEnvelope, isCancellation, logAppError, toUserMessage } from './appError';

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

	it('detects an un-enveloped cancellation via the message fallback', () => {
		expect(isCancellation(new Error('Processing was cancelled.'))).toBe(true);
	});

	it('returns false for unrelated errors', () => {
		expect(isCancellation(envelope({ category: 'processing', message: 'encode failed' }))).toBe(
			false,
		);
		expect(isCancellation(new Error('disk full'))).toBe(false);
	});
});

describe('logAppError', () => {
	it('logs code and category at error level by default', () => {
		const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
		logAppError('Scope', envelope({ code: 'x', category: 'io' }));
		expect(spy).toHaveBeenCalledWith('Scope code=x category=io');
		spy.mockRestore();
	});

	it('logs at warn level when requested', () => {
		const spy = vi.spyOn(console, 'warn').mockImplementation(() => {});
		logAppError('Purge', envelope({ code: 'y', category: 'resource' }), 'warn');
		expect(spy).toHaveBeenCalledWith('Purge code=y category=resource');
		spy.mockRestore();
	});
});
