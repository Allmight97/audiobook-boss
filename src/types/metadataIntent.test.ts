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
			date: 0,
			cover_art: [],
		});

		expect(patch).toEqual({
			title: { op: 'clear' },
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
			title: '',
			date: 0,
			series_part: '3.5',
			cover_art: [],
		});
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
			series: '',
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
});
