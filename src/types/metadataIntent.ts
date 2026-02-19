import type { AudiobookMetadata } from './metadata';

export const METADATA_INTENT_FIELDS = [
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

export type MetadataIntentField = (typeof METADATA_INTENT_FIELDS)[number];
export type MetadataIntentValueMap = Pick<AudiobookMetadata, MetadataIntentField>;
type MetadataIntentValue<K extends MetadataIntentField> = NonNullable<MetadataIntentValueMap[K]>;
type MetadataIntentWriteValue = MetadataIntentValue<MetadataIntentField>;
type CompiledMetadataPatch = Partial<Record<MetadataIntentField, MetadataIntentWriteValue>>;

export type MetadataFieldIntent<K extends MetadataIntentField = MetadataIntentField> =
	| {
			op: 'set';
			value: MetadataIntentValue<K>;
	  }
	| {
			op: 'clear';
	  };

export type MetadataIntentPatch = Partial<{
	[K in MetadataIntentField]: MetadataFieldIntent<K>;
}>;

function isMetadataIntentField(key: string): key is MetadataIntentField {
	return (METADATA_INTENT_FIELDS as readonly string[]).includes(key);
}

function toClearValue(key: MetadataIntentField): MetadataIntentWriteValue {
	if (key === 'date') {
		return 0;
	}
	if (key === 'cover_art') {
		return [];
	}
	return '';
}

function normalizeStringInput(value: string): string {
	return value.trim();
}

function isNumberArray(value: unknown): value is number[] {
	return Array.isArray(value) && value.every((entry) => typeof entry === 'number');
}

export function hasActionableMetadataIntentPatch(
	patch: MetadataIntentPatch | null | undefined,
): patch is MetadataIntentPatch {
	return Boolean(patch && Object.keys(patch).length > 0);
}

export function mergeMetadataIntentPatches(
	base: MetadataIntentPatch,
	next: MetadataIntentPatch,
): MetadataIntentPatch {
	if (!hasActionableMetadataIntentPatch(base)) {
		return { ...next };
	}
	if (!hasActionableMetadataIntentPatch(next)) {
		return { ...base };
	}
	return { ...base, ...next };
}

export function compileMetadataIntentPatch(patch: MetadataIntentPatch): Partial<AudiobookMetadata> {
	const metadata: CompiledMetadataPatch = {};
	for (const key of METADATA_INTENT_FIELDS) {
		const intent = patch[key];
		if (!intent) {
			continue;
		}
		if (intent.op === 'clear') {
			metadata[key] = toClearValue(key);
			continue;
		}
		metadata[key] = intent.value;
	}
	return metadata as Partial<AudiobookMetadata>;
}

export function applyMetadataIntentPatch(
	base: Partial<AudiobookMetadata>,
	patch: MetadataIntentPatch,
): Partial<AudiobookMetadata> {
	return {
		...base,
		...compileMetadataIntentPatch(patch),
	};
}

export function buildMetadataIntentPatchFromMetadata(
	metadata: Partial<AudiobookMetadata>,
): MetadataIntentPatch {
	const patch: MetadataIntentPatch = {};
	for (const [rawKey, value] of Object.entries(metadata)) {
		if (!isMetadataIntentField(rawKey)) {
			continue;
		}
		const key = rawKey;
		if (value == null) {
			patch[key] = { op: 'clear' };
			continue;
		}
		if (key === 'date') {
			if (typeof value !== 'number') {
				continue;
			}
			patch[key] = value === 0 ? { op: 'clear' } : { op: 'set', value };
			continue;
		}
		if (key === 'cover_art') {
			if (!isNumberArray(value)) {
				continue;
			}
			patch[key] = value.length === 0 ? { op: 'clear' } : { op: 'set', value: [...value] };
			continue;
		}
		if (typeof value !== 'string') {
			continue;
		}
		const normalized = normalizeStringInput(value);
		patch[key] = normalized.length === 0 ? { op: 'clear' } : { op: 'set', value: normalized };
	}
	return patch;
}
