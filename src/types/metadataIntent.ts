import type { AudiobookMetadata } from './metadata';

const METADATA_INTENT_FIELDS = [
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
	'album_sort',
	'cover_art',
	// Compatibility/provenance artifact fields (#281). Kept out of
	// METADATA_DRAFT_FIELDS: normal form saves preserve them; only explicit
	// artifact clear intent touches them (the inspect/clear UI surface was
	// removed pending re-ideation; the backend clear path stays contractual).
	'comment',
	'track',
	'disk',
] as const;

export type MetadataIntentField = (typeof METADATA_INTENT_FIELDS)[number];
export type MetadataIntentValueMap = Pick<AudiobookMetadata, MetadataIntentField>;
/** Track/disk positions cross IPC as `[number, total|null]`; the deep
 * null-to-optional mapping degrades tuples to arrays, so pin them here. */
export type MetadataPositionValue = [number, number | null];
type MetadataIntentValue<K extends MetadataIntentField> = K extends 'track' | 'disk'
	? MetadataPositionValue
	: NonNullable<MetadataIntentValueMap[K]>;

type MetadataSetClearIntent<K extends MetadataIntentField> =
	| {
			op: 'set';
			value: MetadataIntentValue<K>;
	  }
	| {
			op: 'clear';
	  };

export type MetadataFieldIntent<K extends MetadataIntentField = MetadataIntentField> =
	| MetadataSetClearIntent<K>
	| (K extends 'album_sort'
			? {
					op: 'recompute';
				}
			: never);

/** The fields a user asked to change; an absent field keeps its source value. */
export type MetadataIntentPatch = Partial<{
	[K in MetadataIntentField]: MetadataFieldIntent<K>;
}>;
