import type { AudiobookMetadata } from '../types/metadata';
import type { MetadataIntentPatch } from '../types/metadataIntent';
import {
	applyMetadataIntentPatch,
	buildMetadataIntentPatchFromMetadata,
	hasActionableMetadataIntentPatch,
	mergeMetadataIntentPatches,
} from '../types/metadataIntent';

export const METADATA_DRAFT_FIELDS = [
	'title',
	'artist',
	'album',
	'composer',
	'genre',
	'date',
	'description',
	'series',
	'series_part',
	'subseries',
	'subseries_part',
	'cover_art',
] as const;

export type MetadataDraftField = (typeof METADATA_DRAFT_FIELDS)[number];
export type MetadataDraft = Partial<Pick<AudiobookMetadata, MetadataDraftField>>;

export function toMetadataDraft(metadata: Partial<AudiobookMetadata>): MetadataDraft {
	const draft: MetadataDraft = {};
	for (const key of METADATA_DRAFT_FIELDS) {
		if (key in metadata) {
			(draft as Partial<Record<MetadataDraftField, unknown>>)[key] = metadata[key];
		}
	}
	return draft;
}

export function buildMetadataDraftIntent(
	metadata: Partial<AudiobookMetadata>,
): MetadataIntentPatch {
	return buildMetadataIntentPatchFromMetadata(toMetadataDraft(metadata));
}

export function hasActionableMetadataDraftIntent(
	patch: MetadataIntentPatch | null | undefined,
): patch is MetadataIntentPatch {
	return hasActionableMetadataIntentPatch(patch);
}

export function mergeMetadataDraftIntents(
	base: MetadataIntentPatch,
	next: MetadataIntentPatch,
): MetadataIntentPatch {
	return mergeMetadataIntentPatches(base, next);
}

export function applyMetadataDraftIntent(
	base: Partial<AudiobookMetadata>,
	patch: MetadataIntentPatch,
): Partial<AudiobookMetadata> {
	return applyMetadataIntentPatch(base, patch);
}
