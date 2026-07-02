import type {
	MetadataIntentFieldError as GeneratedMetadataIntentFieldError,
	MetadataIntentPatch as GeneratedMetadataIntentPatch,
	MetadataIntentValidationField as GeneratedMetadataIntentValidationField,
	MetadataIntentValidationResult as GeneratedMetadataIntentValidationResult,
} from '../lib/generated/tauri';
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

type MetadataSetClearNoopIntent<K extends MetadataIntentField> =
	| {
			op: 'set';
			value: MetadataIntentValue<K>;
	  }
	| {
			op: 'clear';
	  }
	| {
			op: 'noop';
	  };

export type MetadataFieldIntent<K extends MetadataIntentField = MetadataIntentField> =
	| MetadataSetClearNoopIntent<K>
	| (K extends 'album_sort'
			? {
					op: 'recompute';
				}
			: never);

export type MetadataIntentPatch = Partial<{
	[K in MetadataIntentField]: MetadataFieldIntent<K>;
}>;

export type MetadataIntentValidationField = GeneratedMetadataIntentValidationField;
export type MetadataIntentFieldError = GeneratedMetadataIntentFieldError;
export type MetadataIntentValidationResult = GeneratedMetadataIntentValidationResult;

type MetadataIntentPatchRecord = Partial<Record<MetadataIntentField, MetadataFieldIntent>>;

function setMetadataIntent(
	patch: MetadataIntentPatch,
	key: MetadataIntentField,
	intent: MetadataFieldIntent,
): void {
	(patch as MetadataIntentPatchRecord)[key] = intent;
}

function isMetadataIntentField(key: string): key is MetadataIntentField {
	return (METADATA_INTENT_FIELDS as readonly string[]).includes(key);
}

function normalizeStringInput(value: string): string {
	return value.trim();
}

function isNumberArray(value: unknown): value is number[] {
	return Array.isArray(value) && value.every((entry) => typeof entry === 'number');
}

function isPositionValue(value: unknown): value is [number, number | null | undefined] {
	return Array.isArray(value) && value.length >= 1 && typeof value[0] === 'number';
}

export function hasActionableMetadataIntentPatch(
	patch: MetadataIntentPatch | null | undefined,
): patch is MetadataIntentPatch {
	if (!patch) {
		return false;
	}
	return Object.values(patch).some((intent) => intent && intent.op !== 'noop');
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

function toGeneratedPatchOp(
	intent: Exclude<MetadataFieldIntent, { op: 'noop' }>,
): { op: 'set'; value: unknown } | { op: 'clear' } | { op: 'recompute' } | { op: 'noop' } {
	if (intent.op === 'clear') {
		return { op: 'clear' };
	}
	if (intent.op === 'recompute') {
		return { op: 'recompute' };
	}
	return { op: 'set', value: intent.value };
}

export function compileMetadataIntentPatch(
	patch: MetadataIntentPatch,
): GeneratedMetadataIntentPatch {
	const compiled: Record<string, unknown> = {};
	for (const key of METADATA_INTENT_FIELDS) {
		const intent = patch[key];
		if (!intent || intent.op === 'noop') {
			continue;
		}
		compiled[key] = toGeneratedPatchOp(intent);
	}
	return compiled as GeneratedMetadataIntentPatch;
}

export function applyMetadataIntentPatch(
	base: Partial<AudiobookMetadata>,
	patch: MetadataIntentPatch,
): Partial<AudiobookMetadata> {
	const next: Partial<AudiobookMetadata> = { ...base };
	for (const key of METADATA_INTENT_FIELDS) {
		const intent = patch[key];
		if (!intent || intent.op === 'noop') {
			continue;
		}
		if (intent.op === 'clear') {
			delete next[key];
			continue;
		}
		if (intent.op === 'recompute') {
			continue;
		}
		(next as Record<MetadataIntentField, unknown>)[key] = intent.value;
	}
	return next;
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
			setMetadataIntent(patch, key, { op: 'clear' });
			continue;
		}
		if (key === 'date') {
			if (typeof value !== 'string') {
				continue;
			}
			const trimmed = value.trim();
			if (trimmed.length === 0) {
				setMetadataIntent(patch, key, { op: 'clear' });
				continue;
			}
			setMetadataIntent(patch, key, { op: 'set', value: trimmed });
			continue;
		}
		if (key === 'cover_art') {
			if (!isNumberArray(value)) {
				continue;
			}
			setMetadataIntent(
				patch,
				key,
				value.length === 0 ? { op: 'clear' } : { op: 'set', value: [...value] },
			);
			continue;
		}
		if (key === 'track' || key === 'disk') {
			if (!isPositionValue(value)) {
				continue;
			}
			// Zero position is the backend clear sentinel (#281).
			setMetadataIntent(
				patch,
				key,
				value[0] === 0 ? { op: 'clear' } : { op: 'set', value: [value[0], value[1] ?? null] },
			);
			continue;
		}
		if (typeof value !== 'string') {
			continue;
		}
		const normalized = normalizeStringInput(value);
		setMetadataIntent(
			patch,
			key,
			normalized.length === 0 ? { op: 'clear' } : { op: 'set', value: normalized },
		);
	}
	return patch;
}
