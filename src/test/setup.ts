import '@testing-library/jest-dom/vitest';
import { afterEach, beforeEach, vi } from 'vitest';
import { fakeEngine, resetFakeEngine } from './fixtures/fakeEngine';
import type { TauriCommandName } from './tauriCommandNames';

type TestEventHandler = (event: { event: string; id: number; payload: unknown }) => void;
const eventListeners = new Map<string, Set<TestEventHandler>>();

function emitTestEvent(event: string, payload: unknown): void {
	const handlers = eventListeners.get(event);
	if (!handlers) return;
	for (const handler of handlers) {
		handler({ event, id: Date.now(), payload });
	}
}

vi.mock('@tauri-apps/api/core', () => ({
	convertFileSrc: (path: string, protocol: string) =>
		`${protocol}://localhost/${encodeURIComponent(path)}`,
	invoke: vi.fn().mockImplementation((cmd: TauriCommandName, _args?: unknown) => {
		const engineArgs = _args as {
			client: number;
			sequence: number;
			intent: never;
			filePaths: string[];
		};
		switch (cmd) {
			case 'attach_frontend':
				return fakeEngine().attach();
			case 'session_dispatch':
				return fakeEngine().sessionDispatch(
					engineArgs.client,
					engineArgs.sequence,
					engineArgs.intent,
				);
			case 'settings_dispatch':
				return fakeEngine().settingsDispatch(
					engineArgs.client,
					engineArgs.sequence,
					engineArgs.intent,
				);
			default:
				throw new Error(`[Test Mock] Unhandled Tauri invoke: ${cmd}`);
		}
	}),
	// oxlint-disable-next-line typescript/no-extraneous-class -- the generated bindings import Channel; tests never construct one.
	Channel: class MockChannel {},
}));

vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn().mockImplementation((event: string, handler: TestEventHandler) => {
		if (!eventListeners.has(event)) {
			eventListeners.set(event, new Set());
		}
		eventListeners.get(event)?.add(handler);
		return Promise.resolve(() => {
			eventListeners.get(event)?.delete(handler);
		});
	}),
	emit: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
	open: vi.fn().mockResolvedValue(null),
}));

vi.mock('@tauri-apps/plugin-opener', () => ({
	openUrl: vi.fn().mockResolvedValue(undefined),
}));

Object.defineProperty(window, '__TAURI_INTERNALS__', {
	value: undefined,
	writable: true,
});

function startFakeEngine(): void {
	resetFakeEngine();
	const engine = fakeEngine();
	engine.analyze = () => {
		throw new Error('seed analyze');
	};
	void engine.listenSessionUpdates((update) => emitTestEvent('session-update', update));
}

startFakeEngine();

beforeEach(() => {
	eventListeners.clear();
	startFakeEngine();
});

afterEach(() => {
	eventListeners.clear();
});
