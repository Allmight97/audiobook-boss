import type { MetadataIntentPatch as GeneratedMetadataIntentPatch } from '../lib/generated/tauri';
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

export type MetadataFieldIntent<K extends MetadataIntentField = MetadataIntentField> =
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

export type MetadataIntentPatch = Partial<{
	[K in MetadataIntentField]: MetadataFieldIntent<K>;
}>;

function isMetadataIntentField(key: string): key is MetadataIntentField {
	return (METADATA_INTENT_FIELDS as readonly string[]).includes(key);
}

function normalizeStringInput(value: string): string {
	return value.trim();
}

function isNumberArray(value: unknown): value is number[] {
	return Array.isArray(value) && value.every((entry) => typeof entry === 'number');
}

function normalizePublicationDateInput(value: string): string | null {
	const trimmed = value.trim();
	if (/^\d{4}$/.test(trimmed)) {
		return trimmed;
	}
	const match = trimmed.match(/^(\d{4})-(\d{2})(?:[-T ].*)?$/);
	if (!match) {
		return null;
	}
	const month = Number.parseInt(match[2], 10);
	if (!Number.isInteger(month) || month < 1 || month > 12) {
		return null;
	}
	return `${match[1]}-${match[2]}`;
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
): { op: 'set'; value: unknown } | { op: 'clear' } | { op: 'noop' } {
	if (intent.op === 'clear') {
		return { op: 'clear' };
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
			patch[key] = { op: 'clear' };
			continue;
		}
		if (key === 'date') {
			if (typeof value !== 'string') {
				continue;
			}
			const trimmed = value.trim();
			if (trimmed.length === 0) {
				patch[key] = { op: 'clear' };
				continue;
			}
			const normalized = normalizePublicationDateInput(trimmed);
			if (normalized) {
				patch[key] = { op: 'set', value: normalized };
			}
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
