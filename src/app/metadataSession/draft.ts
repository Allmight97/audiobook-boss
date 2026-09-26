import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { buildMetadataIntentPatchFromMetadata } from '../../types/metadataIntent';

const METADATA_DRAFT_FIELDS = [
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

type MetadataDraftField = (typeof METADATA_DRAFT_FIELDS)[number];
type MetadataDraft = Partial<Pick<AudiobookMetadata, MetadataDraftField>>;

function toMetadataDraft(metadata: Partial<AudiobookMetadata>): MetadataDraft {
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
