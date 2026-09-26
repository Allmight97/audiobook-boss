import type { MetadataFormState } from './fields';
import { formValue } from './form';

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

export function calculateTSOA(series: string, part: string, title: string): string {
	const trimmedSeries = series.trim();
	const trimmedTitle = title.trim();
	if (!trimmedSeries || !trimmedTitle) return '';
	const n = parseInt(part, 10);
	const paddedPart = Number.isNaN(n) || n < 1 ? '00' : n < 10 ? `0${n}` : `${n}`;
	return `${trimmedSeries} ${paddedPart} - ${trimmedTitle}`;
}

export function projectTagPreviewValues(form: MetadataFormState): TagPreviewValues {
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
		tsoa: calculateTSOA(series, part, title),
		year: formValue(form, 'meta-year'),
		genre: formValue(form, 'meta-genre'),
	};
}
