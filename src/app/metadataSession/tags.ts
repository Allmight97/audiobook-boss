import type { MetadataFieldId, MetadataFormState } from './fields';

function formValue(form: MetadataFormState, inputId: MetadataFieldId): string {
	return form.fields[inputId].value.trim();
}

export type TagField =
	| 'title'
	| 'album'
	| 'artist'
	| 'albumArtist'
	| 'composer'
	| 'series'
	| 'part'
	| 'subseries'
	| 'subpart'
	| 'tsoa'
	| 'year'
	| 'genre';

export type TagPreviewValues = Record<TagField, string>;

/** `albumSort` is the TSOA value Rust reports processing would write. */
export function projectTagPreviewValues(
	form: MetadataFormState,
	albumSort: string,
): TagPreviewValues {
	const title = formValue(form, 'meta-title');
	const author = formValue(form, 'meta-author');
	const series = formValue(form, 'meta-series');
	const part = formValue(form, 'meta-series-part');
	return {
		title,
		album: title,
		artist: author,
		albumArtist: author,
		composer: formValue(form, 'meta-narrator'),
		series,
		part,
		subseries: formValue(form, 'meta-subseries'),
		subpart: formValue(form, 'meta-subseries-part'),
		tsoa: albumSort,
		year: formValue(form, 'meta-year'),
		genre: formValue(form, 'meta-genre'),
	};
}
