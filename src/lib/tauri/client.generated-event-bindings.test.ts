import { describe, expectTypeOf, it } from 'vitest';

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
