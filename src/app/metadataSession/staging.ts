import type { AudioFile } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import {
	hasActionableMetadataIntentPatch,
	type MetadataIntentPatch,
} from '../../types/metadataIntent';
import { isUsableMetadataCache, type MetadataCache } from './cache';
import { composeFormIntent, type MetadataFormState } from './form';
import { validateMetadataIntent, type ValidateMetadataIntentPatch } from './validation';

export type PreparedMetadataDraft = {
	readonly targets: ReadonlyArray<AudioFile>;
	readonly intentPatch: MetadataIntentPatch;
	readonly snapshotsByPath: Readonly<Record<string, Partial<AudiobookMetadata>>>;
};

export type PrepareMetadataDraftsResult =
	| { readonly status: 'ready'; readonly prepared: PreparedMetadataDraft | null }
	| { readonly status: 'invalid'; readonly message: string }
	| { readonly status: 'noTarget' };

/**
 * Validates the form's edit intent once and resolves the valid selected files
 * it applies to. Invalid inputs never receive intent, and they never change
 * which fields the form's mode projects.
 */
export async function prepareMetadataDrafts(options: {
	readonly form: MetadataFormState;
	readonly files: ReadonlyArray<AudioFile>;
	readonly validate: ValidateMetadataIntentPatch;
	readonly readUncachedMetadata: (file: AudioFile) => Promise<Partial<AudiobookMetadata> | null>;
}): Promise<PrepareMetadataDraftsResult> {
	const intentPatch = composeFormIntent(options.form);
	if (!hasActionableMetadataIntentPatch(intentPatch)) {
		return { status: 'ready', prepared: null };
	}
	const targets = options.files.filter((file) => file.isValid);
	if (targets.length === 0) {
		return { status: 'noTarget' };
	}
	const validation = await validateMetadataIntent(intentPatch, options.validate);
	if (!validation.ok) {
		return { status: 'invalid', message: validation.errors.first ?? 'Metadata validation failed.' };
	}
	const snapshotsByPath: Record<string, Partial<AudiobookMetadata>> = {};
	await Promise.all(
		targets.map(async (file) => {
			const snapshot = await options.readUncachedMetadata(file);
			if (snapshot) snapshotsByPath[file.path] = snapshot;
		}),
	);
	return {
		status: 'ready',
		prepared: { targets, intentPatch: validation.intentPatch, snapshotsByPath },
	};
}

export function commitPreparedMetadataDrafts(
	prepared: PreparedMetadataDraft,
	cache: MetadataCache,
): void {
	for (const [path, metadata] of Object.entries(prepared.snapshotsByPath)) {
		if (!isUsableMetadataCache(cache.getMetadataForFile(path))) {
			cache.cacheMetadataForFile(path, metadata);
		}
	}
	for (const file of prepared.targets) {
		cache.stageMetadataIntentPatch(file.path, prepared.intentPatch);
	}
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
