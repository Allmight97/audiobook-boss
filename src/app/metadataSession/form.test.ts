import { describe, expect, it } from 'vitest';
import { createEmptyCoverUiState } from './cover';
import { createEmptyFormState, replaceField } from './fields';
import {
	hasDirtyMetadataFields,
	populateMetadataFormMulti,
	populateMetadataFormSingle,
	readMetadataForm,
} from './form';

describe('metadata form projection', () => {
	it('maps single-mode fields including album alias and cover bytes', () => {
		const form = populateMetadataFormSingle({
			title: 'Title',
			artist: 'Author',
			composer: 'Narrator',
			date: '2024-07',
			genre: 'Fiction',
			series: 'Series',
			series_part: '2',
			subseries: 'Sub-series',
			subseries_part: '4',
			description: 'Desc',
		});
		const metadata = readMetadataForm(form, {
			mode: 'single',
			coverArtBytes: [1, 2, 3],
		});
		expect(metadata).toMatchObject({
			title: 'Title',
			album: 'Title',
			artist: 'Author',
			composer: 'Narrator',
			date: '2024-07',
			cover_art: [1, 2, 3],
		});
	});

	it('emits empty cover_art when removal is requested', () => {
		const metadata = readMetadataForm(createEmptyFormState(), {
			coverArtRemovalRequested: true,
		});
		expect(metadata.cover_art).toEqual([]);
		expect(
			hasDirtyMetadataFields(createEmptyFormState(), {
				...createEmptyCoverUiState(),
				coverArtRemovalRequested: true,
			}),
		).toBe(true);
	});

	it('uses bulk blank and ignores cover in multi mode', () => {
		let form = populateMetadataFormMulti([{ title: 'A' }, { title: 'B' }], 2);
		form = replaceField(form, 'meta-year', { action: 'blank', dirty: true });
		const metadata = readMetadataForm(form, {
			mode: 'multi',
			onlyDirty: true,
			coverArtBytes: [1, 2, 3],
			coverArtRemovalRequested: true,
		});
		expect(metadata.date).toBeUndefined();
		expect(metadata.cover_art).toBeUndefined();
	});
});
