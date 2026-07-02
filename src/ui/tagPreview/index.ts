/**
 * Tag Preview module
 *
 * Calculates TSOA (sort key) and updates the tag preview grid
 * based on metadata form preview state.
 */
import { readMetadataFormPreviewValues } from '../metadataForm';
import {
	TAG_FIELDS,
	createEmptyTagPreviewValues,
	setTagPreviewValues,
	type TagField,
	type TagPreviewValues,
} from './state.svelte';

/**
 * Pads a part number to 2 digits for proper sorting
 */
function padPart(num: string): string {
	const n = parseInt(num, 10);
	if (Number.isNaN(n) || n < 1) return '00';
	return n < 10 ? `0${n}` : `${n}`;
}

/**
 * Calculates the TSOA (Album Sort) tag value
 * Format: "Series PP - Title" where PP is zero-padded part number
 * Returns empty string if series or title is missing
 */
export function calculateTSOA(series: string, part: string, title: string): string {
	const trimmedSeries = series.trim();
	const trimmedTitle = title.trim();

	if (!trimmedSeries || !trimmedTitle) return '';

	const paddedPart = padPart(part);
	return `${trimmedSeries} ${paddedPart} - ${trimmedTitle}`;
}

/**
 * Tag field mappings from metadata form preview values to tag preview fields.
 */
type MetadataFormPreviewValues = ReturnType<typeof readMetadataFormPreviewValues>;

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

function getTagPreviewValues(): TagPreviewValues {
	const preview = readMetadataFormPreviewValues();
	const values = createEmptyTagPreviewValues();
	for (const field of TAG_FIELDS) {
		values[field] = TAG_FIELD_MAPPINGS[field](preview);
	}
	return values;
}

/**
 * Updates all tag preview fields
 */
export function updateTagPreview(): void {
	setTagPreviewValues(getTagPreviewValues());
}

/**
 * Initializes tag preview island and synchronizes with the current metadata preview state.
 */
export function initTagPreview(): void {
	updateTagPreview();
}
export { default as TagPreviewIsland } from './TagPreviewIsland.svelte';
