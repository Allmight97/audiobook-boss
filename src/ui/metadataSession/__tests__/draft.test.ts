import { describe, expect, it } from 'vitest';

import {
	applyMetadataDraftIntent,
	buildMetadataDraftIntent,
	hasActionableMetadataDraftIntent,
	toMetadataDraft,
} from '../draft';

describe('metadata draft intent', () => {
	it('keeps normal UI drafts inside the supported write surface', () => {
		const draft = toMetadataDraft({
			title: ' Draft Title ',
			album_sort: 'Curated Sort',
			comment: 'Reader note',
			track: [1, 12],
			disk: [1, 1],
		});

		expect(draft).toEqual({ title: ' Draft Title ' });
	});

	it('does not emit album sort or read-only fields from normal draft metadata', () => {
		const patch = buildMetadataDraftIntent({
			title: 'Book',
			album_sort: 'Curated Sort',
			comment: 'Reader note',
			track: [1, 12],
			disk: [1, 1],
		});

		expect(patch).toEqual({
			title: { op: 'set', value: 'Book' },
		});
	});

	it('preserves clear intent when applying draft patches', () => {
		const patch = buildMetadataDraftIntent({ title: '', cover_art: [] });

		expect(hasActionableMetadataDraftIntent(patch)).toBe(true);
		expect(applyMetadataDraftIntent({ title: 'Old', artist: 'Author' }, patch)).toEqual({
			artist: 'Author',
		});
	});
});
