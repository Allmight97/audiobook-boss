import { describe, expect, it } from 'vitest';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { createMetadataCache } from './cache';

const path = '/books/a.m4b';
const author = { artist: { op: 'set', value: 'FDK Decision' } } as const;
const title = { title: { op: 'set', value: 'NMR 64k' } } as const;

describe('metadata cache save commit', () => {
	it('does not discard an explicit clear when the source value is unknown', async () => {
		const cache = createMetadataCache();
		const clear = { artist: { op: 'clear' } } as const;
		cache.stageMetadataIntentPatch(path, clear);
		expect(cache.getMetadataIntentPatchForFile(path)).toEqual(clear);

		cache.clear();
		await cache.readSourceMetadata(path, async () => ({ title: 'Known coverless book' }));
		cache.stageMetadataIntentPatch(path, clear);
		expect(cache.getMetadataIntentPatchForFile(path)).toBeUndefined();
	});

	it('folds a saved patch into the source tags and stops tracking it', async () => {
		const cache = createMetadataCache();
		await cache.readSourceMetadata(path, async () => ({ title: 'Old', artist: 'Source Author' }));
		cache.stageMetadataIntentPatch(path, author);
		const saved = cache.getMetadataIntentPatchForFile(path);

		cache.commitSavedIntent(path, saved!);

		expect(cache.getMetadataIntentPatchForFile(path)).toBeUndefined();
		expect(cache.getMetadataForFile(path)).toEqual({ title: 'Old', artist: 'FDK Decision' });
	});

	it('keeps every pending edit when another was staged while the save ran', async () => {
		const cache = createMetadataCache();
		await cache.readSourceMetadata(path, async () => ({ title: 'Old', artist: 'Source Author' }));
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

	it('a first complete read after an unread save adds unknown tags and keeps newer edits', async () => {
		const cache = createMetadataCache();
		cache.stageMetadataIntentPatch(path, author);
		cache.commitSavedIntent(path, cache.getMetadataIntentPatchForFile(path)!);
		cache.stageMetadataIntentPatch(path, title);

		await cache.readSourceMetadata(path, async () => ({
			title: 'Old',
			artist: 'FDK Decision',
			genre: 'Fantasy',
		}));

		expect(cache.hasSourceMetadata(path)).toBe(true);
		expect(cache.getMetadataForFile(path)).toEqual({
			title: 'NMR 64k',
			artist: 'FDK Decision',
			genre: 'Fantasy',
		});
	});

	it.each(['removal', 'reset'] as const)(
		'a late read cannot revive tags after %s, and reimport reads fresh tags',
		async (change) => {
			const cache = createMetadataCache();
			let finish!: (value: { title: string }) => void;
			const pending = cache.readSourceMetadata(
				path,
				() =>
					new Promise((resolve) => {
						finish = resolve;
					}),
			);
			if (change === 'removal') cache.dropRemovedPaths(new Set());
			else cache.clear();
			finish({ title: 'Removed' });
			expect(await pending).toBeUndefined();
			expect(cache.hasSourceMetadata(path)).toBe(false);
			expect(await cache.readSourceMetadata(path, async () => ({ title: 'Reimported' }))).toEqual({
				title: 'Reimported',
			});
		},
	);
});
