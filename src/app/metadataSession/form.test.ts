import { describe, expect, it } from 'vitest';
import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataFieldId } from './fields';
import {
	applyFieldAction,
	applyFieldInput,
	composeFormIntent,
	populateMetadataFormMulti,
	populateMetadataFormSingle,
	type MetadataFormState,
} from './form';
import { replaceField } from './fields';

const tagged: Partial<AudiobookMetadata> = {
	title: 'Title',
	artist: 'Author',
	date: '2024-07',
	series: 'Saga',
	series_part: '7/8',
	cover_art: [1, 2, 3],
};

function type(form: MetadataFormState, inputId: MetadataFieldId, value: string) {
	return applyFieldInput(replaceField(form, inputId, { value }), inputId);
}

describe('composeFormIntent', () => {
	it.each([
		{ name: 'untouched single title', form: () => populateMetadataFormSingle(tagged), intent: {} },
		{
			name: 'edited title mirrors album',
			form: () => type(populateMetadataFormSingle(tagged), 'meta-title', ' New '),
			intent: { title: { op: 'set', value: 'New' }, album: { op: 'set', value: 'New' } },
		},
		{
			name: 'emptied date clears',
			form: () => type(populateMetadataFormSingle(tagged), 'meta-year', ''),
			intent: { date: { op: 'clear' } },
		},
		{
			name: 'emptied mixed field is a bulk blank',
			form: () =>
				type(populateMetadataFormMulti([tagged, { artist: 'Other' }], 2), 'meta-author', ''),
			intent: { artist: { op: 'clear' } },
		},
		{
			name: 'Keep after Blank restores the shared value',
			form: () =>
				applyFieldAction(
					applyFieldAction(populateMetadataFormMulti([tagged, tagged], 2), 'meta-series', 'blank'),
					'meta-series',
					'keep',
				),
			intent: {},
		},
	])('$name', ({ form, intent }) => {
		expect(composeFormIntent(form())).toEqual(intent);
	});
});
