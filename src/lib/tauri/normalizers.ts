import type {
	AudiobookMetadata as GeneratedAudiobookMetadata,
	FileListInfo as GeneratedFileListInfo,
	OnlineMetadataResult as GeneratedOnlineMetadataResult,
	ProcessV2Payload as GeneratedProcessV2Payload,
	ProgressEvent as GeneratedProgressEvent,
	QueueEvent as GeneratedQueueEvent,
} from '../generated/tauri';
import type {
	FileListInfo,
	ProcessCommandJobResult,
	ProcessCommandResult,
	ProcessV2Payload,
} from '../../types/audio';
import type { AudiobookMetadata, OnlineMetadataResult } from '../../types/metadata';
import type { ProcessingProgressEvent, ProcessingQueueEvent } from '../../types/events';

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
	'sampleRate',
	'jobType',
	'outputNaming',
] as const satisfies readonly (keyof GeneratedProcessV2Payload)[];

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

export function normalizeMetadata(metadata: GeneratedAudiobookMetadata): AudiobookMetadata {
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

export function normalizeFileList(info: GeneratedFileListInfo): FileListInfo {
	return normalizeNullish(info) as FileListInfo;
}

export function normalizeLookupResult(result: GeneratedOnlineMetadataResult): OnlineMetadataResult {
	return normalizeNullish(result) as OnlineMetadataResult;
}

export function denormalizeProcessPayload(payload: ProcessV2Payload): GeneratedProcessV2Payload {
	const nullableFields = toNullableShape<
		GeneratedProcessV2Payload,
		(typeof PROCESS_PAYLOAD_NULLABLE_FIELDS)[number]
	>(
		payload as Partial<Record<(typeof PROCESS_PAYLOAD_NULLABLE_FIELDS)[number], unknown>>,
		PROCESS_PAYLOAD_NULLABLE_FIELDS,
	);
	return {
		inputFiles: payload.inputFiles,
		outputDir: payload.outputDir,
		settings: payload.settings as GeneratedProcessV2Payload['settings'],
		...nullableFields,
	};
}

export function normalizeProcessResult(result: unknown): ProcessCommandResult {
	const normalized = normalizeNullish(result) as Record<string, unknown>;
	const defaultMessage =
		typeof normalized.message === 'string' ? normalized.message : 'Processing completed';
	const rawResults = Array.isArray(normalized.results)
		? normalized.results
		: [
				{
					jobId: normalized.jobId,
					message: normalized.message,
					previewFilePath: normalized.previewFilePath,
					previewActualSeconds: normalized.previewActualSeconds,
					success: true,
				},
			];

	const results: ProcessCommandJobResult[] = rawResults
		.filter(isPlainRecord)
		.map((entry) => ({
			jobId: typeof entry.jobId === 'string' ? entry.jobId : undefined,
			message: typeof entry.message === 'string' ? entry.message : undefined,
			stage: typeof entry.stage === 'string' ? entry.stage : undefined,
			success: typeof entry.success === 'boolean' ? entry.success : undefined,
			outputFilePath: typeof entry.outputFilePath === 'string' ? entry.outputFilePath : undefined,
			previewFilePath:
				typeof entry.previewFilePath === 'string' ? entry.previewFilePath : undefined,
			previewActualSeconds:
				typeof entry.previewActualSeconds === 'number' ? entry.previewActualSeconds : undefined,
		}))
		.filter((entry) => Object.values(entry).some((value) => value !== undefined));

	return {
		message: defaultMessage,
		results,
	};
}

export function normalizeProgressEvent(payload: GeneratedProgressEvent): ProcessingProgressEvent {
	const normalized = normalizeNullish(payload) as ProcessingProgressEvent;
	return {
		...normalized,
		stage: payload.stage as ProcessingProgressEvent['stage'],
	};
}

export function normalizeQueueEvent(payload: GeneratedQueueEvent): ProcessingQueueEvent {
	return normalizeNullish(payload) as ProcessingQueueEvent;
}
