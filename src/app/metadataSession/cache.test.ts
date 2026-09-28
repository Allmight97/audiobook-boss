import { describe, expect, it } from 'vitest';
import { createMetadataCache } from './cache';

const path = '/books/a.m4b';
const author = { artist: { op: 'set', value: 'FDK Decision' } } as const;
const title = { title: { op: 'set', value: 'NMR 64k' } } as const;

describe('metadata cache save commit', () => {
	it('folds a saved patch into the source tags and stops tracking it', () => {
		const cache = createMetadataCache();
		cache.recordSourceMetadata(path, { title: 'Old', artist: 'Source Author' });
		cache.stageMetadataIntentPatch(path, author);
		const saved = cache.getMetadataIntentPatchForFile(path);

		cache.commitSavedIntent(path, saved!);

		expect(cache.getMetadataIntentPatchForFile(path)).toBeUndefined();
		expect(cache.getMetadataForFile(path)).toEqual({ title: 'Old', artist: 'FDK Decision' });
	});

	it('keeps every pending edit when another was staged while the save ran', () => {
		const cache = createMetadataCache();
		cache.recordSourceMetadata(path, { title: 'Old', artist: 'Source Author' });
		cache.stageMetadataIntentPatch(path, author);
		const saved = cache.getMetadataIntentPatchForFile(path);
		cache.stageMetadataIntentPatch(path, title);

		cache.commitSavedIntent(path, saved!);

		expect(cache.getMetadataIntentPatchForFile(path)).toEqual({ ...author, ...title });
		expect(cache.getMetadataForFile(path)).toEqual({ title: 'NMR 64k', artist: 'FDK Decision' });
	});
});
