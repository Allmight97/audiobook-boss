/**
 * IPC payload normalizers for the Rust → TypeScript boundary.
 *
 * Each public normalizer accepts a specta-generated payload type from
 * `../generated/tauri` (where Rust `Option<T>` surfaces as `T | null`) and
 * returns the matching UI-friendly type from `../../types/*` (where `null`
 * has been converted to optional via `NullToOptionalDeep`). This keeps the
 * adapter layer thin and well-typed: input drift is caught at compile time,
 * and the runtime transform is centralized in `normalizeNullish`.
 *
 * Inverse direction: `denormalize*` helpers rebuild payloads with explicit
 * `null` values for the wire (see `denormalizeMetadata`,
 * `denormalizeProcessPayload`, and `denormalizeNullish`).
 */

import type {
	AudiobookMetadata as GeneratedAudiobookMetadata,
	EncoderAvailability as GeneratedEncoderAvailability,
	FileListInfo as GeneratedFileListInfo,
	MetadataSaveBatchResult as GeneratedMetadataSaveBatchResult,
	MetadataLookupResponse as GeneratedMetadataLookupResponse,
	OnlineMetadataResult as GeneratedOnlineMetadataResult,
	ProcessCommandResult as GeneratedProcessCommandResult,
	ProcessPayload as GeneratedProcessPayload,
	ProgressEvent as GeneratedProgressEvent,
	QueueEvent as GeneratedQueueEvent,
	RuntimeSettingsCapabilities as GeneratedRuntimeSettingsCapabilities,
} from '../generated/tauri';
import type {
	EncoderAvailability,
	FileListInfo,
	ProcessCommandResult,
	ProcessPayload,
	RuntimeSettingsCapabilities,
} from '../../types/audio';
import type {
	AudiobookMetadata,
	MetadataLookupResponse,
	MetadataSaveBatchResult,
	OnlineMetadataResult,
} from '../../types/metadata';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../types/events';
import type { NullToOptionalDeep } from '../../types/ipc';
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
	'collisionPolicy',
	'preflightSignature',
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

/**
 * Recursively strips `null` from a generated Tauri payload so fields that were
 * typed as `T | null` on the wire become `T | undefined` (optional) in app code.
 *
 * The return type is `NullToOptionalDeep<T>` — the type-level twin of this
 * runtime transform. Typing the return this way means every downstream
 * normalizer can return a UI-friendly type (e.g. `AudiobookMetadata =
 * NullToOptionalDeep<GeneratedAudiobookMetadata>`) without an `as` cast at
 * the call site.
 */
export function normalizeNullish<T>(value: T): NullToOptionalDeep<T> {
	if (value == null) {
		return undefined as NullToOptionalDeep<T>;
	}
	if (Array.isArray(value)) {
		if (isScalarArrayWithoutNullish(value)) {
			return value as NullToOptionalDeep<T>;
		}
		const normalized = new Array<unknown>(value.length);
		for (const [index, entry] of value.entries()) {
			normalized[index] = normalizeNullish(entry);
		}
		return normalized as NullToOptionalDeep<T>;
	}
	if (isPlainRecord(value)) {
		const normalized: PlainRecord = {};
		for (const [key, entryValue] of Object.entries(value)) {
			const converted = normalizeNullish(entryValue);
			if (converted !== undefined) {
				normalized[key] = converted;
			}
		}
		return normalized as NullToOptionalDeep<T>;
	}
	return value as NullToOptionalDeep<T>;
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

export function normalizeMetadata(metadata: GeneratedAudiobookMetadata): AudiobookMetadata {
	return normalizeNullish(metadata);
}

export function denormalizeMetadata(
	metadata: Partial<AudiobookMetadata>,
): GeneratedAudiobookMetadata {
	return toNullableShape<GeneratedAudiobookMetadata, (typeof METADATA_FIELDS)[number]>(
		metadata as Partial<Record<(typeof METADATA_FIELDS)[number], unknown>>,
		METADATA_FIELDS,
	) as GeneratedAudiobookMetadata;
}

export function normalizeFileList(info: GeneratedFileListInfo): FileListInfo {
	return normalizeNullish(info);
}

export function normalizeEncoderAvailability(
	availability: GeneratedEncoderAvailability,
): EncoderAvailability {
	return normalizeNullish(availability);
}

export function normalizeRuntimeSettingsCapabilities(
	capabilities: GeneratedRuntimeSettingsCapabilities,
): RuntimeSettingsCapabilities {
	return normalizeNullish(capabilities);
}

export function normalizeLookupResult(result: GeneratedOnlineMetadataResult): OnlineMetadataResult {
	return normalizeNullish(result);
}

export function normalizeLookupResponse(
	response: GeneratedMetadataLookupResponse,
): MetadataLookupResponse {
	return {
		results: response.results.map(normalizeLookupResult),
		diagnostics: response.diagnostics.map(normalizeNullish),
	};
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
		settings: payload.settings,
		...nullableFields,
	};
}

export function normalizeProcessResult(
	result: GeneratedProcessCommandResult,
): ProcessCommandResult {
	const normalized = normalizeNullish(result) as ProcessCommandResult;
	return {
		...normalized,
		results: (normalized.results ?? []).map((entry) => ({
			...entry,
			error: entry.error == null ? undefined : normalizeAppError(entry.error),
		})),
	};
}

export function normalizeMetadataSaveBatchResult(
	result: GeneratedMetadataSaveBatchResult,
): MetadataSaveBatchResult {
	const normalized = normalizeNullish(result) as MetadataSaveBatchResult;
	return {
		...normalized,
		results: (normalized.results ?? []).map((entry) => ({
			...entry,
			error: entry.error == null ? undefined : normalizeAppError(entry.error),
		})),
	};
}

export function normalizeProgressEvent(payload: GeneratedProgressEvent): ProcessingProgressEvent {
	return normalizeNullish(payload);
}

export function normalizeQueueEvent(payload: GeneratedQueueEvent): ProcessingQueueEvent {
	return normalizeNullish(payload);
}
