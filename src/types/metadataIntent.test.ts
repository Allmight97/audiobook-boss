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
			cover_art: { op: 'clear' },
		});

		expect(payload).toEqual({
			title: { op: 'clear' },
			date: { op: 'clear' },
			series_part: { op: 'set', value: '3.5' },
			cover_art: { op: 'clear' },
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
			{ title: 'Old', artist: 'Author', series: 'Series A' },
			{
				title: { op: 'set', value: 'New' },
				series: { op: 'clear' },
			},
		);

		expect(merged).toEqual({
			title: 'New',
			artist: 'Author',
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

	it('normalizes publication dates to YYYY or YYYY-MM', () => {
		const patch = buildMetadataIntentPatchFromMetadata({
			date: '2024-07-15',
		});

		expect(patch).toEqual({
			date: { op: 'set', value: '2024-07' },
		});
	});

	it('ignores read-only track and disk fields when building write intent', () => {
		const patch = buildMetadataIntentPatchFromMetadata({
			title: 'Writable',
			track: [3, 12],
			disk: [1, 2],
		} as unknown as Parameters<typeof buildMetadataIntentPatchFromMetadata>[0]);

		expect(patch).toEqual({
			title: { op: 'set', value: 'Writable' },
		});
	});
});
