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
	it('maps single-mode fields including album alias without cover bytes', () => {
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
		});
		expect(metadata).toMatchObject({
			title: 'Title',
			album: 'Title',
			artist: 'Author',
			composer: 'Narrator',
			date: '2024-07',
		});
		expect(metadata.cover_art).toBeUndefined();
	});

	it('treats cover removal as dirty without emitting cover bytes on the form', () => {
		expect(readMetadataForm(createEmptyFormState()).cover_art).toBeUndefined();
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
			coverArtRemovalRequested: true,
		});
		expect(metadata.date).toBeUndefined();
		expect(metadata.cover_art).toBeUndefined();
	});
});
