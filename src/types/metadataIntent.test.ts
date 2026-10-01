import { describe, expect, it } from 'vitest';
import { compileMetadataIntentPatch } from './metadataIntent';

describe('metadata intent patch compilation', () => {
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
});
