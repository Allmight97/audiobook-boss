import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const logFrontendMock = vi.fn().mockResolvedValue(undefined);

vi.mock('./tauri/client', () => ({
	tauriClient: { commands: { logFrontend: (...args: unknown[]) => logFrontendMock(...args) } },
}));

type Bridge = typeof import('./frontendLogBridge');

function rejectionEvent(reason: unknown): PromiseRejectionEvent {
	return new PromiseRejectionEvent('unhandledrejection', {
		promise: Promise.reject().catch(() => undefined),
		reason,
	});
}

describe('frontend error log bridge', () => {
	let bridge: Bridge;
	let registered: Array<[string, EventListenerOrEventListenerObject]> = [];

	function setTauri(present: boolean): void {
		(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = present ? {} : undefined;
	}

	async function freshBridge(): Promise<Bridge> {
		vi.resetModules();
		return import('./frontendLogBridge');
	}

	function removeRegisteredListeners(): void {
		for (const [type, listener] of registered) window.removeEventListener(type, listener);
		registered = [];
	}

	beforeEach(async () => {
		logFrontendMock.mockClear();
		const addEventListener = window.addEventListener.bind(window);
		vi.spyOn(window, 'addEventListener').mockImplementation(
			(type: string, listener: EventListenerOrEventListenerObject, options?: unknown) => {
				registered.push([type, listener]);
				addEventListener(type, listener, options as AddEventListenerOptions);
			},
		);
		setTauri(true);
		bridge = await freshBridge();
		bridge.initFrontendErrorLogBridge();
	});

	afterEach(() => {
		removeRegisteredListeners();
		vi.restoreAllMocks();
		setTauri(false);
	});

	it('does not install outside a Tauri webview', async () => {
		removeRegisteredListeners();
		setTauri(false);
		(await freshBridge()).initFrontendErrorLogBridge();
		window.dispatchEvent(rejectionEvent(new Error('boom')));
		expect(logFrontendMock).not.toHaveBeenCalled();
	});

	it('redacts arbitrary rejection values', () => {
		window.dispatchEvent(rejectionEvent('token=sk-secret /Users/me/private/file.m4b'));
		const entry = logFrontendMock.mock.calls[0][0];
		expect(entry.message).not.toContain('sk-secret');
		expect(entry.message).not.toContain('/Users/me');
		expect(entry.message).toContain('String rejection value');
	});

	it('reports fixed-window overflow after rollover', () => {
		const nowSpy = vi.spyOn(Date, 'now').mockReturnValue(0);
		for (let i = 0; i < 25; i++) window.dispatchEvent(rejectionEvent(new Error(`e-${i}`)));
		expect(logFrontendMock).toHaveBeenCalledTimes(20);
		nowSpy.mockReturnValue(120_000);
		window.dispatchEvent(rejectionEvent(new Error('after')));
		expect(logFrontendMock).toHaveBeenCalledTimes(22);
		expect(logFrontendMock.mock.calls[20][0]).toEqual({
			level: 'warn',
			scope: 'frontendLogBridge.rateLimit',
			message: 'Dropped 5 frontend log event(s) over the 20/60s fixed-window limit.',
		});
		nowSpy.mockRestore();
	});

	it('forwards an error under its name and installs once', () => {
		bridge.initFrontendErrorLogBridge();
		window.dispatchEvent(rejectionEvent(new Error('boom')));
		expect(logFrontendMock).toHaveBeenCalledTimes(1);
		expect(logFrontendMock.mock.calls[0][0]).toEqual({
			level: 'error',
			scope: 'window.unhandledrejection:Error',
			message: 'boom',
		});
	});

	it('forwards a scoped failure with only the code and category of an app error', () => {
		bridge.logFrontendFailure('Scope', {
			code: 'x',
			category: 'io',
			message: '/Users/me/secret.m4b',
			detail: 'token=sk-abc',
		});
		expect(logFrontendMock).toHaveBeenCalledWith({
			level: 'error',
			scope: 'Scope',
			message: 'code=x category=io',
		});
	});

	it('forwards a scoped failure under the shared rate limit', () => {
		const nowSpy = vi.spyOn(Date, 'now').mockReturnValue(0);
		for (let i = 0; i < 25; i++) bridge.logFrontendFailure('Scope', new Error(`e-${i}`));
		expect(logFrontendMock).toHaveBeenCalledTimes(20);
		expect(logFrontendMock.mock.calls[0][0]).toEqual({
			level: 'error',
			scope: 'Scope',
			message: 'Error: e-0',
		});
		nowSpy.mockRestore();
	});
});
