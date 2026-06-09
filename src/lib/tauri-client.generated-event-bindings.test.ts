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
		await tauriClient.listen(EVENTS.OPENED_AUDIO_FILES, () => {
			/* no-op */
		});
		await tauriClient.listen(EVENTS.WORK_OPERATION_SNAPSHOT, () => {
			/* no-op */
		});
		await tauriClient.listen(EVENTS.WORK_OPERATION_LIST_SNAPSHOT, () => {
			/* no-op */
		});

		expect(tauriListen).toHaveBeenNthCalledWith(1, EVENTS.PROGRESS, expect.any(Function));
		expect(tauriListen).toHaveBeenNthCalledWith(2, EVENTS.QUEUE, expect.any(Function));
		expect(tauriListen).toHaveBeenNthCalledWith(3, EVENTS.OPENED_AUDIO_FILES, expect.any(Function));
		expect(tauriListen).toHaveBeenNthCalledWith(
			4,
			EVENTS.WORK_OPERATION_SNAPSHOT,
			expect.any(Function),
		);
		expect(tauriListen).toHaveBeenNthCalledWith(
			5,
			EVENTS.WORK_OPERATION_LIST_SNAPSHOT,
			expect.any(Function),
		);
		expect(tauriListen).toHaveBeenCalledTimes(5);
	});
});
