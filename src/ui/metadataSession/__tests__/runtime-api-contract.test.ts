import { beforeEach, describe, expect, it } from 'vitest';

import * as metadataSession from '..';

const EXPECTED_METADATA_SESSION_EXPORTS = [
	'buildMetadataDraftIntent',
	'cacheMetadataForFile',
	'clearMetadataSession',
	'collectActionableMetadataIntent',
	'getMetadataForFile',
	'getMetadataIntentPatchForFile',
	'isUsableMetadataCache',
	'metadataSaveInProgress',
	'removeMetadataForFile',
	'saveMetadataFromUI',
	'stageMetadataIntentPatch',
	'validateMetadataDraft',
] as const;

describe('Metadata Session runtime public API contract', () => {
	beforeEach(() => {
		metadataSession.clearMetadataSession();
	});

	it('pins the metadata session public export strip', () => {
		expect(Object.keys(metadataSession).sort()).toEqual(
			[...EXPECTED_METADATA_SESSION_EXPORTS].sort(),
		);
	});

	it('stages intent through the outcome call and reports it as actionable pending intent', () => {
		const path = '/books/contract.m4b';

		expect(
			metadataSession.stageMetadataIntentPatch(path, { title: { op: 'set', value: 'Pinned' } }),
		).toBe('staged');
		expect(metadataSession.collectActionableMetadataIntent([path])).toEqual({
			[path]: { title: { op: 'set', value: 'Pinned' } },
		});

		// Same patch again: cache already reflects the merge — no new pending truth.
		expect(
			metadataSession.stageMetadataIntentPatch(path, { title: { op: 'set', value: 'Pinned' } }),
		).toBe('unchanged');

		// A patch with no actionable ops never touches pending state.
		expect(metadataSession.stageMetadataIntentPatch(path, {})).toBe('noop');

		metadataSession.clearMetadataSession();
		expect(metadataSession.collectActionableMetadataIntent([path])).toBeNull();
	});

	it('keeps artifact fields out of draft staging while allowing explicit clears', () => {
		const path = '/books/artifact.m4b';
		metadataSession.cacheMetadataForFile(path, {
			title: 'Old Title',
			album_sort: 'Keep Me',
			comment: 'Keep Comment',
			track: [3, 12],
			disk: [1, null],
		});

		// Normal draft staging: draft intent building filters artifact fields out,
		// so a form edit can never wipe album_sort/comment/track/disk.
		const draftIntent = metadataSession.buildMetadataDraftIntent({
			title: 'New Title',
			album_sort: 'Sneaky Overwrite',
			comment: 'Sneaky Comment',
		});
		expect(draftIntent).not.toHaveProperty('album_sort');
		expect(draftIntent).not.toHaveProperty('comment');

		expect(metadataSession.stageMetadataIntentPatch(path, draftIntent)).toBe('staged');
		const cached = metadataSession.getMetadataForFile(path);
		expect(cached?.title).toBe('New Title');
		expect(cached?.album_sort).toBe('Keep Me');
		expect(cached?.comment).toBe('Keep Comment');
		expect(cached?.track).toEqual([3, 12]);
		expect(cached?.disk).toEqual([1, null]);

		// Explicit artifact clears flow through the same staging seam as clear ops.
		expect(metadataSession.stageMetadataIntentPatch(path, { album_sort: { op: 'clear' } })).toBe(
			'staged',
		);
		expect(metadataSession.getMetadataForFile(path)?.album_sort).toBeUndefined();
		expect(metadataSession.collectActionableMetadataIntent([path])?.[path]).toMatchObject({
			album_sort: { op: 'clear' },
		});
	});
});
