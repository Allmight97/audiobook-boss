/**
 * Tag Preview module
 *
 * Calculates TSOA (sort key) and updates the tag preview grid
 * based on metadata form preview state.
 */
import { metadataFormPreviewState } from './metadataForm/previewState.svelte';
import {
	TAG_FIELDS,
	createEmptyTagPreviewValues,
	setTagPreviewValues,
	type TagField,
	type TagPreviewValues,
} from './tagPreview/state.svelte';

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
 * Tag field mappings from metadata form preview state to tag preview fields.
 */
const TAG_FIELD_MAPPINGS: Record<TagField, () => string> = {
	title: () => metadataFormPreviewState.title,
	album: () => metadataFormPreviewState.title,
	artist: () => metadataFormPreviewState.author,
	albumArtist: () => metadataFormPreviewState.author,
	composer: () => metadataFormPreviewState.narrator,
	series: () => metadataFormPreviewState.series,
	part: () => metadataFormPreviewState.seriesPart,
	subseries: () => metadataFormPreviewState.subseries,
	subpart: () => metadataFormPreviewState.subseriesPart,
	year: () => metadataFormPreviewState.year,
	genre: () => metadataFormPreviewState.genre,
	tsoa: () =>
		calculateTSOA(
			metadataFormPreviewState.series,
			metadataFormPreviewState.seriesPart,
			metadataFormPreviewState.title,
		),
};

function getTagPreviewValues(): TagPreviewValues {
	const values = createEmptyTagPreviewValues();
	for (const field of TAG_FIELDS) {
		values[field] = TAG_FIELD_MAPPINGS[field]();
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
export { default as TagPreviewIsland } from './tagPreview/TagPreviewIsland.svelte';
