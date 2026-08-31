import type { AudioFile } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import type { MetadataIntentPatch } from '../../types/metadataIntent';
import {
	cacheMetadataForFile,
	getMetadataForFile,
	isUsableMetadataCache,
	stageMetadataIntentPatch,
} from './cache';
import type { CoverUiState } from './cover';
import { coverIntentFromUi } from './cover';
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
	});
	const validation = await validateMetadataDraft(
		metadata,
		options.validate,
		coverIntentFromUi(options.cover),
	);
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

export function commitPreparedMetadataDrafts(prepared: PreparedMetadataDraft): boolean {
	if (prepared.kind === 'none') return true;
	if (prepared.kind === 'single') {
		stageMetadataIntentPatch(prepared.filePath, prepared.intentPatch);
		return true;
	}
	for (const [path, metadata] of Object.entries(prepared.pendingCacheByPath)) {
		if (!isUsableMetadataCache(getMetadataForFile(path))) {
			cacheMetadataForFile(path, metadata);
		}
	}
	const stageResults = prepared.files.map((file) =>
		stageMetadataIntentPatch(file.path, prepared.intentPatch),
	);
	return stageResults[0] !== 'noop';
}

export function resetFormAfterCommit(form: MetadataFormState): MetadataFormState {
	return resetDirtyState(form);
}

export async function readUncachedMetadataSnapshot(
	file: AudioFile,
	readAudioMetadata: (path: string) => Promise<Partial<AudiobookMetadata>>,
): Promise<Partial<AudiobookMetadata> | null> {
	if (!file.isValid) return null;
	if (isUsableMetadataCache(getMetadataForFile(file.path))) return null;
	try {
		return await readAudioMetadata(file.path);
	} catch (error) {
		console.warn('Failed to load metadata:', error);
		return null;
	}
}
