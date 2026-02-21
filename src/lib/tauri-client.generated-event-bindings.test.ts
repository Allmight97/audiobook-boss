import { beforeEach, describe, expect, it, vi } from 'vitest';

import { EVENTS } from '../types/events';

describe('tauriClient generated event bindings', () => {
	beforeEach(() => {
		vi.resetModules();
		vi.clearAllMocks();
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
	});

	it('routes app events through generated tauri listeners', async () => {
		const { listen } = await import('@tauri-apps/api/event');
		const tauriListen = vi.mocked(listen);
		const { tauriClient } = await import('./tauri/client');

		await tauriClient.listen(EVENTS.PROGRESS, () => {
			/* no-op */
		});
		await tauriClient.listen(EVENTS.QUEUE, () => {
			/* no-op */
		});

		expect(tauriListen).toHaveBeenNthCalledWith(1, EVENTS.PROGRESS, expect.any(Function));
		expect(tauriListen).toHaveBeenNthCalledWith(2, EVENTS.QUEUE, expect.any(Function));
		expect(tauriListen).toHaveBeenCalledTimes(2);
	});
});
