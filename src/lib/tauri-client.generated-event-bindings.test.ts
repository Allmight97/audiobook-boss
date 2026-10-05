import { beforeEach, describe, expect, expectTypeOf, it, vi } from 'vitest';

import { EVENTS } from '../types/events';
import type {
	AcquisitionProgress,
	ChapterSpec,
	ChildJobSnapshot,
	MaterializedSourceFile,
	MaxConcurrentJobsCapabilities,
	OperationSnapshot,
	ProgressSnapshot,
	SelectionSnapshot,
	SessionUpdate,
} from './generated/tauri';

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

		await tauriClient.listen(EVENTS.SESSION_UPDATE, () => {
			/* no-op */
		});
		await tauriClient.listen(EVENTS.WORK_OPERATIONS_UPDATE, () => {
			/* no-op */
		});

		expect(tauriListen).toHaveBeenNthCalledWith(1, EVENTS.SESSION_UPDATE, expect.any(Function));
		expect(tauriListen).toHaveBeenNthCalledWith(
			2,
			EVENTS.WORK_OPERATIONS_UPDATE,
			expect.any(Function),
		);
		expect(tauriListen).toHaveBeenCalledTimes(2);
	});

	it('keeps bounded wide Rust values in the numeric IPC contract', () => {
		expectTypeOf<AcquisitionProgress['percentage']>().toEqualTypeOf<number>();
		expectTypeOf<ChapterSpec['startMs']>().toEqualTypeOf<number>();
		expectTypeOf<MaterializedSourceFile['sizeBytes']>().toEqualTypeOf<number>();
		expectTypeOf<OperationSnapshot['sequence']>().toEqualTypeOf<number>();
		expectTypeOf<ChildJobSnapshot['startedAtMs']>().toEqualTypeOf<number | null>();
		expectTypeOf<ChildJobSnapshot['finishedAtMs']>().toEqualTypeOf<number | null>();
		expectTypeOf<SessionUpdate['revision']>().toEqualTypeOf<number>();
		expectTypeOf<SelectionSnapshot['selectedIndices']>().toEqualTypeOf<number[]>();
		expectTypeOf<ProgressSnapshot['percentage']>().toEqualTypeOf<number>();
		expectTypeOf<ProgressSnapshot['bytesDownloaded']>().toEqualTypeOf<number | null>();
		expectTypeOf<MaxConcurrentJobsCapabilities['fixedMax']>().toEqualTypeOf<number>();
	});
});

describe('remote snapshot normalization', () => {
	it('normalizes cached library title optionals and preserves meaningful empty account/job snapshots', async () => {
		const { normalizeSessionUpdate } = await import('./tauri/normalizers');
		const { fakeRemote } = await import('../test/fixtures/fakeEngine');
		const seed = fakeRemote();
		const remote = {
			...seed,
			account: null,
			acquisition: null,
			indexer: { ...seed.indexer, releases: [] },
		};
		const update = normalizeSessionUpdate({
			revision: 3,
			titles: null,
			selection: null,
			metadata: null,
			lookup: null,
			audio: null,
			output: null,
			remote,
			remoteLibrary: {
				revision: 2,
				titles: [
					{
						providerId: 'audible',
						titleId: 'book',
						title: 'Book',
						authors: [],
						narrators: [],
						durationSeconds: null,
						coverUrl: null,
						supplementalPdfAvailable: false,
						acquired: false,
						availability: {
							status: 'available',
							acquirable: true,
							label: 'Available',
							detail: null,
						},
						unsupportedReasons: [],
					},
				],
				diagnostics: [{ kind: 'validationFailed', titleId: null, message: 'Partial library' }],
			},
		});
		expect(update.remote?.account).toBeNull();
		expect(update.remote?.acquisition).toBeNull();
		expect(update.remoteLibrary?.titles[0]).not.toHaveProperty('coverUrl');
		expect(update.remoteLibrary?.titles[0].availability).not.toHaveProperty('detail');
		expect(update.remoteLibrary?.diagnostics[0]).not.toHaveProperty('titleId');
		const progressOnly = normalizeSessionUpdate({
			revision: 4,
			titles: null,
			selection: null,
			metadata: null,
			lookup: null,
			audio: null,
			output: null,
			remote,
			remoteLibrary: null,
		});
		expect(progressOnly.remoteLibrary).toBeUndefined();
	});
});
