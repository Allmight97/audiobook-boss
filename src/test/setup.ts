import '@testing-library/jest-dom/vitest';
import { afterEach, beforeEach, vi } from 'vitest';
import { fakeEngine, resetFakeEngine } from './fixtures/fakeEngine';
import { invokeMock } from './fixtures/invokeMock';

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
	invoke: invokeMock,
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

if (typeof Element.prototype.setPointerCapture !== 'function') {
	Element.prototype.setPointerCapture = () => {};
}

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
