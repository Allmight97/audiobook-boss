import type { MetadataFormPreviewValues } from './form';

export const TAG_FIELDS = [
	'title',
	'album',
	'artist',
	'albumArtist',
	'composer',
	'series',
	'part',
	'subseries',
	'subpart',
	'tsoa',
	'year',
	'genre',
] as const;

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

const EMPTY_VALUES: TagPreviewValues = {
	title: '',
	album: '',
	artist: '',
	albumArtist: '',
	composer: '',
	series: '',
	part: '',
	subseries: '',
	subpart: '',
	tsoa: '',
	year: '',
	genre: '',
};

export function createEmptyTagPreviewValues(): TagPreviewValues {
	return { ...EMPTY_VALUES };
}

export function calculateTSOA(series: string, part: string, title: string): string {
	const trimmedSeries = series.trim();
	const trimmedTitle = title.trim();
	if (!trimmedSeries || !trimmedTitle) return '';
	const n = parseInt(part, 10);
	const paddedPart = Number.isNaN(n) || n < 1 ? '00' : n < 10 ? `0${n}` : `${n}`;
	return `${trimmedSeries} ${paddedPart} - ${trimmedTitle}`;
}

const TAG_FIELD_MAPPINGS: Record<TagField, (preview: MetadataFormPreviewValues) => string> = {
	title: (preview) => preview.title,
	album: (preview) => preview.title,
	artist: (preview) => preview.author,
	albumArtist: (preview) => preview.author,
	composer: (preview) => preview.narrator,
	series: (preview) => preview.series,
	part: (preview) => preview.seriesPart,
	subseries: (preview) => preview.subseries,
	subpart: (preview) => preview.subseriesPart,
	year: (preview) => preview.year,
	genre: (preview) => preview.genre,
	tsoa: (preview) => calculateTSOA(preview.series, preview.seriesPart, preview.title),
};

export function projectTagPreviewValues(preview: MetadataFormPreviewValues): TagPreviewValues {
	const values = createEmptyTagPreviewValues();
	for (const field of Object.keys(TAG_FIELD_MAPPINGS) as TagField[]) {
		values[field] = TAG_FIELD_MAPPINGS[field](preview);
	}
	return values;
}
