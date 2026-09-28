import { describe, expect, it } from 'vitest';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
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

	it('keeps saved values when the file could not be read, and a later clear still stages', () => {
		const cache = createMetadataCache();
		const cover: MetadataIntentPatch = { cover_art: { op: 'set', value: [9, 9, 9] } };
		cache.stageMetadataIntentPatch(path, { ...author, ...cover });
		const saved = cache.getMetadataIntentPatchForFile(path);

		cache.commitSavedIntent(path, saved!);

		expect(cache.getMetadataForFile(path)).toEqual({
			artist: 'FDK Decision',
			cover_art: [9, 9, 9],
		});
		expect(cache.hasSourceMetadata(path)).toBe(false);
		cache.stageMetadataIntentPatch(path, { artist: { op: 'clear' } });
		expect(cache.getMetadataIntentPatchForFile(path)).toEqual({ artist: { op: 'clear' } });
	});

	it('a first complete read after an unread save adds unknown tags and keeps newer edits', () => {
		const cache = createMetadataCache();
		cache.stageMetadataIntentPatch(path, author);
		cache.commitSavedIntent(path, cache.getMetadataIntentPatchForFile(path)!);
		cache.stageMetadataIntentPatch(path, title);

		cache.recordSourceMetadata(path, { title: 'Old', artist: 'FDK Decision', genre: 'Fantasy' });

		expect(cache.hasSourceMetadata(path)).toBe(true);
		expect(cache.getMetadataForFile(path)).toEqual({
			title: 'NMR 64k',
			artist: 'FDK Decision',
			genre: 'Fantasy',
		});
	});
});
