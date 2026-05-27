import { describe, expect, it } from 'vitest';

import {
	applyMetadataIntentPatch,
	buildMetadataIntentPatchFromMetadata,
	compileMetadataIntentPatch,
	hasActionableMetadataIntentPatch,
	mergeMetadataIntentPatches,
} from './metadataIntent';

describe('metadata intent patch helpers', () => {
	it('builds clear intents for empty editable values', () => {
		const patch = buildMetadataIntentPatchFromMetadata({
			title: '',
			date: '',
			cover_art: [],
		});

		expect(patch).toEqual({
			title: { op: 'clear' },
			date: { op: 'clear' },
			cover_art: { op: 'clear' },
		});
	});

	it('builds clear intents for explicit undefined values', () => {
		const patch = buildMetadataIntentPatchFromMetadata({
			date: undefined,
			cover_art: undefined,
		});

		expect(patch).toEqual({
			date: { op: 'clear' },
			cover_art: { op: 'clear' },
		});
	});

	it('compiles clear and set operations into backend-compatible values', () => {
		const payload = compileMetadataIntentPatch({
			title: { op: 'clear' },
			date: { op: 'clear' },
			series_part: { op: 'set', value: '3.5' },
			album_sort: { op: 'set', value: 'Series 03 - Title' },
			cover_art: { op: 'clear' },
		});

		expect(payload).toEqual({
			title: { op: 'clear' },
			date: { op: 'clear' },
			series_part: { op: 'set', value: '3.5' },
			album_sort: { op: 'set', value: 'Series 03 - Title' },
			cover_art: { op: 'clear' },
		});
	});

	it('compiles album sort clear and recompute operations explicitly', () => {
		expect(
			compileMetadataIntentPatch({
				album_sort: { op: 'clear' },
			}),
		).toEqual({
			album_sort: { op: 'clear' },
		});
		expect(
			compileMetadataIntentPatch({
				album_sort: { op: 'recompute' },
			}),
		).toEqual({
			album_sort: { op: 'recompute' },
		});
	});

	it('omits noop operations when compiling backend payloads', () => {
		const payload = compileMetadataIntentPatch({
			title: { op: 'noop' },
			artist: { op: 'clear' },
			date: { op: 'set', value: '2024-07' },
		});

		expect(payload).toEqual({
			artist: { op: 'clear' },
			date: { op: 'set', value: '2024-07' },
		});
		expect('title' in payload).toBe(false);
	});

	it('applies patches on top of existing metadata', () => {
		const merged = applyMetadataIntentPatch(
			{ title: 'Old', artist: 'Author', series: 'Series A', album_sort: 'Custom Sort' },
			{
				title: { op: 'set', value: 'New' },
				series: { op: 'clear' },
				album_sort: { op: 'recompute' },
			},
		);

		expect(merged).toEqual({
			title: 'New',
			artist: 'Author',
			album_sort: 'Custom Sort',
		});
	});

	it('builds album sort set and clear intents from explicit metadata', () => {
		expect(
			buildMetadataIntentPatchFromMetadata({
				album_sort: 'Custom Sort',
			}),
		).toEqual({
			album_sort: { op: 'set', value: 'Custom Sort' },
		});

		expect(
			buildMetadataIntentPatchFromMetadata({
				album_sort: '',
			}),
		).toEqual({
			album_sort: { op: 'clear' },
		});
	});

	it('merges patches by preferring latest op per field', () => {
		const merged = mergeMetadataIntentPatches(
			{
				title: { op: 'set', value: 'Draft' },
				series: { op: 'set', value: 'Series A' },
			},
			{
				title: { op: 'clear' },
			},
		);
		expect(merged).toEqual({
			title: { op: 'clear' },
			series: { op: 'set', value: 'Series A' },
		});
		expect(hasActionableMetadataIntentPatch(merged)).toBe(true);
	});

	it('preserves publication dates for backend validation and normalization', () => {
		const patch = buildMetadataIntentPatchFromMetadata({
			date: '2024-07-15',
		});

		expect(patch).toEqual({
			date: { op: 'set', value: '2024-07-15' },
		});
	});

	it('preserves invalid publication dates so backend validation can report them', () => {
		const patch = buildMetadataIntentPatchFromMetadata({
			date: 'not a date',
		});

		expect(patch).toEqual({
			date: { op: 'set', value: 'not a date' },
		});
	});

	it('ignores read-only track, disk, and comment fields when building write intent', () => {
		const patch = buildMetadataIntentPatchFromMetadata({
			title: 'Writable',
			track: [3, 12],
			disk: [1, 2],
			comment: 'Reader note',
		} as unknown as Parameters<typeof buildMetadataIntentPatchFromMetadata>[0]);

		expect(patch).toEqual({
			title: { op: 'set', value: 'Writable' },
		});
	});
});
