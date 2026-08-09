import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const logFrontendMock = vi.fn().mockResolvedValue(undefined);

vi.mock('./tauri/client', () => ({
	tauriClient: { logFrontend: (...args: unknown[]) => logFrontendMock(...args) },
}));

import {
	disposeFrontendErrorLogBridgeForTests,
	flattenLogText,
	initFrontendErrorLogBridge,
} from './frontendLogBridge';

function rejectionEvent(reason: unknown): PromiseRejectionEvent {
	return new PromiseRejectionEvent('unhandledrejection', {
		promise: Promise.reject().catch(() => undefined) as unknown as Promise<unknown>,
		reason,
	});
}

describe('frontend error log bridge', () => {
	beforeEach(() => {
		logFrontendMock.mockClear();
		(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {};
		initFrontendErrorLogBridge();
	});

	afterEach(() => {
		disposeFrontendErrorLogBridgeForTests();
		(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = undefined;
	});

	it('does not install outside a Tauri webview', () => {
		disposeFrontendErrorLogBridgeForTests();
		(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = undefined;
		initFrontendErrorLogBridge();
		window.dispatchEvent(rejectionEvent(new Error('boom')));
		expect(logFrontendMock).not.toHaveBeenCalled();
	});

	it('redacts arbitrary rejection values', () => {
		window.dispatchEvent(rejectionEvent('token=sk-secret /Users/me/private/file.m4b'));
		const entry = logFrontendMock.mock.calls[0]![0];
		expect(entry.message).not.toContain('sk-secret');
		expect(entry.message).not.toContain('/Users/me');
		expect(entry.message).toContain('String rejection value');
	});

	it('forwards a bounded error record', () => {
		window.dispatchEvent(rejectionEvent(new Error('x'.repeat(600))));
		const entry = logFrontendMock.mock.calls[0]![0];
		expect(entry.scope).toBe('window.unhandledrejection:Error');
		expect(entry.message.length).toBe(501);
	});

	it('reports fixed-window overflow after rollover', () => {
		const nowSpy = vi.spyOn(Date, 'now').mockReturnValue(0);
		for (let i = 0; i < 25; i++) window.dispatchEvent(rejectionEvent(new Error(`e-${i}`)));
		expect(logFrontendMock).toHaveBeenCalledTimes(20);
		nowSpy.mockReturnValue(120_000);
		window.dispatchEvent(rejectionEvent(new Error('after')));
		expect(logFrontendMock).toHaveBeenCalledTimes(22);
		expect(logFrontendMock.mock.calls[20]![0]).toEqual({
			level: 'warn',
			scope: 'frontendLogBridge.rateLimit',
			message: 'Dropped 5 frontend log event(s) over the 20/60s fixed-window limit.',
		});
		nowSpy.mockRestore();
	});

	it('flattens log-breaking controls and installs once', () => {
		expect(flattenLogText('a\nb\rc\t')).toBe('a b c ');
		initFrontendErrorLogBridge();
		window.dispatchEvent(rejectionEvent(new Error('single')));
		expect(logFrontendMock).toHaveBeenCalledTimes(1);
	});
});
