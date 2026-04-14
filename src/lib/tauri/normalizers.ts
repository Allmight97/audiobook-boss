import type {
	AudiobookMetadata as GeneratedAudiobookMetadata,
	ProcessPayload as GeneratedProcessPayload,
} from '../generated/tauri';
import type {
	EncoderAvailability,
	FileListInfo,
	ProcessCommandResult,
	ProcessPayload,
} from '../../types/audio';
import type { AudiobookMetadata, OnlineMetadataResult } from '../../types/metadata';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../types/events';
import { normalizeAppError } from './appError';

type PlainRecord = Record<string, unknown>;

const METADATA_FIELDS = [
	'title',
	'artist',
	'album',
	'composer',
	'genre',
	'date',
	'track',
	'disk',
	'comment',
	'description',
	'series',
	'series_part',
	'subseries',
	'subseries_part',
	'album_sort',
	'cover_art',
] as const satisfies readonly (keyof GeneratedAudiobookMetadata)[];

const PROCESS_PAYLOAD_NULLABLE_FIELDS = [
	'externalToolchain',
	'sampleRate',
	'jobType',
	'outputNaming',
] as const satisfies readonly (keyof GeneratedProcessPayload)[];

const isPlainRecord = (value: unknown): value is PlainRecord =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

function isScalarArrayWithoutNullish(value: readonly unknown[]): boolean {
	for (const entry of value) {
		if (entry == null || typeof entry === 'object') {
			return false;
		}
	}
	return true;
}

export function normalizeNullish<T>(value: T): T {
	if (value == null) {
		return undefined as T;
	}
	if (Array.isArray(value)) {
		if (isScalarArrayWithoutNullish(value)) {
			return value as T;
		}
		const normalized = new Array<unknown>(value.length);
		for (const [index, entry] of value.entries()) {
			normalized[index] = normalizeNullish(entry);
		}
		return normalized as T;
	}
	if (isPlainRecord(value)) {
		const normalized: PlainRecord = {};
		for (const [key, entryValue] of Object.entries(value)) {
			const converted = normalizeNullish(entryValue);
			if (converted !== undefined) {
				normalized[key] = converted;
			}
		}
		return normalized as T;
	}
	return value;
}

export function denormalizeNullish<T>(value: T): T {
	if (value === undefined) {
		return null as T;
	}
	if (Array.isArray(value)) {
		if (isScalarArrayWithoutNullish(value)) {
			return value as T;
		}
		const normalized = new Array<unknown>(value.length);
		for (const [index, entry] of value.entries()) {
			normalized[index] = denormalizeNullish(entry);
		}
		return normalized as T;
	}
	if (isPlainRecord(value)) {
		const normalized: PlainRecord = {};
		for (const [key, entryValue] of Object.entries(value)) {
			normalized[key] = denormalizeNullish(entryValue);
		}
		return normalized as T;
	}
	return value;
}

function toNullableShape<T extends PlainRecord, K extends keyof T>(
	input: Partial<Record<K, unknown>>,
	keys: readonly K[],
): Pick<T, K> {
	const output = {} as Pick<T, K>;
	for (const key of keys) {
		const value = input[key];
		(output as PlainRecord)[key as string] = value === undefined ? null : denormalizeNullish(value);
	}
	return output;
}

export function normalizeMetadata(metadata: unknown): AudiobookMetadata {
	return normalizeNullish(metadata) as AudiobookMetadata;
}

export function denormalizeMetadata(
	metadata: Partial<AudiobookMetadata>,
): GeneratedAudiobookMetadata {
	return toNullableShape<GeneratedAudiobookMetadata, (typeof METADATA_FIELDS)[number]>(
		metadata as Partial<Record<(typeof METADATA_FIELDS)[number], unknown>>,
		METADATA_FIELDS,
	) as GeneratedAudiobookMetadata;
}

export function normalizeFileList(info: unknown): FileListInfo {
	return normalizeNullish(info) as FileListInfo;
}

export function normalizeEncoderAvailability(availability: unknown): EncoderAvailability {
	return normalizeNullish(availability) as EncoderAvailability;
}

export function normalizeLookupResult(result: unknown): OnlineMetadataResult {
	return normalizeNullish(result) as OnlineMetadataResult;
}

export function denormalizeProcessPayload(payload: ProcessPayload): GeneratedProcessPayload {
	const nullableFields = toNullableShape<
		GeneratedProcessPayload,
		(typeof PROCESS_PAYLOAD_NULLABLE_FIELDS)[number]
	>(
		payload as Partial<Record<(typeof PROCESS_PAYLOAD_NULLABLE_FIELDS)[number], unknown>>,
		PROCESS_PAYLOAD_NULLABLE_FIELDS,
	);
	return {
		inputFiles: payload.inputFiles,
		outputDir: payload.outputDir,
		settings: payload.settings as GeneratedProcessPayload['settings'],
		...nullableFields,
	};
}

export function normalizeProcessResult(result: unknown): ProcessCommandResult {
	const normalized = normalizeNullish(result) as ProcessCommandResult;
	return {
		...normalized,
		results: (normalized.results ?? []).map((entry) => ({
			...entry,
			error: entry.error == null ? undefined : normalizeAppError(entry.error),
		})),
	};
}

export function normalizeProgressEvent(payload: unknown): ProcessingProgressEvent {
	const normalized = normalizeNullish(payload) as ProcessingProgressEvent;
	return {
		...normalized,
		stage: normalized.stage as ProcessingProgressEvent['stage'],
	};
}

export function normalizeQueueEvent(payload: unknown): ProcessingQueueEvent {
	return normalizeNullish(payload) as ProcessingQueueEvent;
}
