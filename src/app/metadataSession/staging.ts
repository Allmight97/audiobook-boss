import type { AudioFile } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import { isUsableMetadataCache, type MetadataCache } from './cache';
import type { CoverUiState } from './cover';
import {
	hasDirtyMetadataFields,
	readMetadataForm,
	resetDirtyState,
	type MetadataFormState,
} from './form';
import { validateMetadataDraft, type ValidateMetadataIntentPatch } from './validation';

export type PreparedMetadataDraft =
	| { readonly kind: 'none' }
	| {
			readonly kind: 'single';
			readonly filePath: string;
			readonly intentPatch: MetadataIntentPatch;
	  }
	| {
			readonly kind: 'multi';
			readonly files: ReadonlyArray<AudioFile>;
			readonly intentPatch: MetadataIntentPatch;
			readonly pendingCacheByPath: Readonly<Record<string, Partial<AudiobookMetadata>>>;
	  };

export type PrepareMetadataDraftsResult =
	| { readonly ok: true; readonly prepared: PreparedMetadataDraft }
	| { readonly ok: false; readonly message: string };

export async function prepareMetadataDrafts(options: {
	readonly form: MetadataFormState;
	readonly cover: CoverUiState;
	readonly selectedFiles: ReadonlyArray<AudioFile>;
	readonly validate: ValidateMetadataIntentPatch;
	readonly readUncachedMetadata: (file: AudioFile) => Promise<Partial<AudiobookMetadata> | null>;
}): Promise<PrepareMetadataDraftsResult> {
	const validFiles = options.selectedFiles.filter((file) => file.isValid);
	if (validFiles.length === 0) {
		return { ok: true, prepared: { kind: 'none' } };
	}
	if (validFiles.length === 1) {
		return prepareSingleSelectionMetadata(validFiles[0], options);
	}
	return prepareMultiSelectionMetadata(validFiles, options);
}

async function prepareSingleSelectionMetadata(
	file: AudioFile | undefined,
	options: {
		readonly form: MetadataFormState;
		readonly cover: CoverUiState;
		readonly validate: ValidateMetadataIntentPatch;
	},
): Promise<PrepareMetadataDraftsResult> {
	if (!file?.isValid || !hasDirtyMetadataFields(options.form, options.cover)) {
		return { ok: true, prepared: { kind: 'none' } };
	}
	const metadata = readMetadataForm(options.form, {
		mode: 'single',
		coverArtBytes: options.cover.currentCoverArt,
		coverArtRemovalRequested: options.cover.coverArtRemovalRequested,
	});
	const validation = await validateMetadataDraft(metadata, options.validate);
	if (!validation.ok) {
		return { ok: false, message: validation.errors.first ?? 'Metadata validation failed.' };
	}
	return {
		ok: true,
		prepared: { kind: 'single', filePath: file.path, intentPatch: validation.intentPatch },
	};
}

async function prepareMultiSelectionMetadata(
	selectedFiles: ReadonlyArray<AudioFile>,
	options: {
		readonly form: MetadataFormState;
		readonly cover: CoverUiState;
		readonly validate: ValidateMetadataIntentPatch;
		readonly readUncachedMetadata: (file: AudioFile) => Promise<Partial<AudiobookMetadata> | null>;
	},
): Promise<PrepareMetadataDraftsResult> {
	const changes = readMetadataForm(options.form, { mode: 'multi', onlyDirty: true });
	if (Object.keys(changes).length === 0) {
		return { ok: true, prepared: { kind: 'none' } };
	}
	const validation = await validateMetadataDraft(changes, options.validate);
	if (!validation.ok) {
		return { ok: false, message: validation.errors.first ?? 'Metadata validation failed.' };
	}
	const pendingCacheByPath: Record<string, Partial<AudiobookMetadata>> = {};
	await Promise.all(
		selectedFiles.map(async (file) => {
			const snapshot = await options.readUncachedMetadata(file);
			if (snapshot) pendingCacheByPath[file.path] = snapshot;
		}),
	);
	return {
		ok: true,
		prepared: {
			kind: 'multi',
			files: selectedFiles,
			intentPatch: validation.intentPatch,
			pendingCacheByPath,
		},
	};
}

export function commitPreparedMetadataDrafts(
	prepared: PreparedMetadataDraft,
	cache: MetadataCache,
): boolean {
	if (prepared.kind === 'none') return true;
	if (prepared.kind === 'single') {
		cache.stageMetadataIntentPatch(prepared.filePath, prepared.intentPatch);
		return true;
	}
	for (const [path, metadata] of Object.entries(prepared.pendingCacheByPath)) {
		if (!isUsableMetadataCache(cache.getMetadataForFile(path))) {
			cache.cacheMetadataForFile(path, metadata);
		}
	}
	const stageResults = prepared.files.map((file) =>
		cache.stageMetadataIntentPatch(file.path, prepared.intentPatch),
	);
	return stageResults[0] !== 'noop';
}

export function resetFormAfterCommit(form: MetadataFormState): MetadataFormState {
	return resetDirtyState(form);
}

export async function readUncachedMetadataSnapshot(
	file: AudioFile,
	readAudioMetadata: (path: string) => Promise<Partial<AudiobookMetadata>>,
	cache: MetadataCache,
): Promise<Partial<AudiobookMetadata> | null> {
	if (!file.isValid) return null;
	if (isUsableMetadataCache(cache.getMetadataForFile(file.path))) return null;
	try {
		return await readAudioMetadata(file.path);
	} catch (error) {
		console.warn('Failed to load metadata:', error);
		return null;
	}
}
