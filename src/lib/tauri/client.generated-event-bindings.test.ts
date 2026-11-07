import { beforeEach, describe, expect, expectTypeOf, it, vi } from 'vitest';

import type {
	AcquisitionProgress,
	ChapterSpec,
	ChildJobSnapshot,
	OperationSnapshot,
	ProgressSnapshot,
	SelectionSnapshot,
	SessionUpdate,
} from '../generated/tauri';

describe('tauriClient generated event bindings', () => {
	beforeEach(() => {
		vi.resetModules();
		vi.clearAllMocks();
		(window as Window & { __TAURI_INTERNALS__?: unknown }).__TAURI_INTERNALS__ = {};
	});

	it('routes every engine event through its generated listener', async () => {
		const { listen } = await import('@tauri-apps/api/event');
		const tauriListen = vi.mocked(listen);
		const { tauriClient } = await import('./client');

		await tauriClient.events.sessionUpdate.listen(() => {});
		await tauriClient.events.settingsUpdate.listen(() => {});
		await tauriClient.events.workOperationsUpdate.listen(() => {});
		await tauriClient.events.quitPressAgain.listen(() => {});

		expect(tauriListen.mock.calls.map(([name]) => name)).toEqual([
			'session-update',
			'settings-update',
			'work-operations-update',
			'quit-press-again',
		]);
	});

	it('keeps bounded wide Rust values in the numeric IPC contract', () => {
		expectTypeOf<AcquisitionProgress['percentage']>().toEqualTypeOf<number>();
		expectTypeOf<ChapterSpec['startMs']>().toEqualTypeOf<number>();
		expectTypeOf<OperationSnapshot['sequence']>().toEqualTypeOf<number>();
		expectTypeOf<ChildJobSnapshot['startedAtMs']>().toEqualTypeOf<number | null>();
		expectTypeOf<ChildJobSnapshot['finishedAtMs']>().toEqualTypeOf<number | null>();
		expectTypeOf<SessionUpdate['revision']>().toEqualTypeOf<number>();
		expectTypeOf<SelectionSnapshot['selectedIndices']>().toEqualTypeOf<number[]>();
		expectTypeOf<ProgressSnapshot['percentage']>().toEqualTypeOf<number>();
		expectTypeOf<AcquisitionProgress['bytesDownloaded']>().toEqualTypeOf<number | null>();
	});
});
