/**
 * Metadata validation adapter: builds draft intent, routes it through the
 * injected Rust boundary validator, and reshapes the result into one
 * outcome callers can branch on without knowing the boundary shape.
 */

import type { AudiobookMetadata } from '../../types/metadata';
import type {
	MetadataIntentPatch,
	MetadataIntentValidationField,
	MetadataIntentValidationResult,
} from '../../types/metadataIntent';
import { buildMetadataDraftIntent } from './draft';

export type ValidateMetadataIntentPatch = (
	patch: MetadataIntentPatch,
) => Promise<MetadataIntentValidationResult>;

export interface MetadataDraftValidation {
	/** Backend-normalized intent patch — stage this, not the built patch. */
	intentPatch: MetadataIntentPatch;
	ok: boolean;
	errors: {
		first: string | null;
		byField: Partial<Record<MetadataIntentValidationField, string>>;
	};
	/** Raw boundary result for callers that render non-blocking warnings. */
	result: MetadataIntentValidationResult;
}

export async function validateMetadataDraft(
	metadata: Partial<AudiobookMetadata>,
	validate: ValidateMetadataIntentPatch,
): Promise<MetadataDraftValidation> {
	const result = await validate(buildMetadataDraftIntent(metadata));
	const byField: Partial<Record<MetadataIntentValidationField, string>> = {};
	for (const error of result.fieldErrors) {
		if (byField[error.field] === undefined) {
			byField[error.field] = error.message;
		}
	}
	return {
		intentPatch: result.metadataPatch,
		ok: result.fieldErrors.length === 0,
		errors: {
			first: result.fieldErrors[0]?.message ?? null,
			byField,
		},
		result,
	};
}
