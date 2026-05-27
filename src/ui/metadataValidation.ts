/**
 * Metadata validation adapters for UI workflows.
 */

import type { AudiobookMetadata } from '../types/metadata';
import type {
	MetadataIntentPatch,
	MetadataIntentValidationField,
	MetadataIntentValidationResult,
} from '../types/metadataIntent';
import { buildMetadataDraftIntent } from './metadataDraft';

export type ValidateMetadataIntentPatch = (
	patch: MetadataIntentPatch,
) => Promise<MetadataIntentValidationResult>;

export interface MetadataDraftIntentValidation {
	intentPatch: MetadataIntentPatch;
	result: MetadataIntentValidationResult;
}

export function getMetadataIntentFieldError(
	result: MetadataIntentValidationResult,
	field: MetadataIntentValidationField,
): string | null {
	return result.fieldErrors.find((error) => error.field === field)?.message ?? null;
}

export function getSeriesPartValidationError(
	result: MetadataIntentValidationResult,
): string | null {
	return getMetadataIntentFieldError(result, 'series_part');
}

export function getSubseriesPartValidationError(
	result: MetadataIntentValidationResult,
): string | null {
	return getMetadataIntentFieldError(result, 'subseries_part');
}

export function firstMetadataIntentValidationError(
	result: MetadataIntentValidationResult,
): string | null {
	return result.fieldErrors[0]?.message ?? null;
}

export async function validateMetadataDraftIntent(
	metadata: Partial<AudiobookMetadata>,
	validateMetadataIntentPatch: ValidateMetadataIntentPatch,
): Promise<MetadataDraftIntentValidation> {
	const intentPatch = buildMetadataDraftIntent(metadata);
	const result = await validateMetadataIntentPatch(intentPatch);
	return {
		intentPatch: result.metadataPatch,
		result,
	};
}
