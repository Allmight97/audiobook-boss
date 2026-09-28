import type { AudioFile } from '../../types/audio';
import type { AudiobookMetadata } from '../../types/metadata';
import {
	hasActionableMetadataIntentPatch,
	type MetadataIntentPatch,
} from '../../types/metadataIntent';
import type { MetadataCache } from './cache';
import { composeFormIntent, type MetadataFormState } from './form';
import { validateMetadataIntent, type ValidateMetadataIntentPatch } from './validation';

type PreparedMetadataDraft = {
	readonly targets: ReadonlyArray<AudioFile>;
	readonly intentPatch: MetadataIntentPatch;
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
	readonly loadMetadata: (file: AudioFile) => Promise<Partial<AudiobookMetadata> | null>;
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
	await Promise.all(targets.map(options.loadMetadata));
	return {
		status: 'ready',
		prepared: { targets, intentPatch: validation.intentPatch },
	};
}

export function commitPreparedMetadataDrafts(
	prepared: PreparedMetadataDraft,
	cache: MetadataCache,
): void {
	for (const file of prepared.targets) {
		cache.stageMetadataIntentPatch(file.path, prepared.intentPatch);
	}
}
