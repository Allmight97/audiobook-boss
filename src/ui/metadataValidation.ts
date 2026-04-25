/**
 * Metadata validation helpers for UI workflows.
 */

const SERIES_PART_INVALID_MESSAGE =
	"Series sequence (#) cannot include '/'. Use a plain number like 24.";

const SUBSERIES_PART_INVALID_MESSAGE =
	"Sub-series sequence (#) cannot include '/'. Use a plain number like 24.";

function getSequenceValidationError(value: string | undefined, message: string): string | null {
	if (!value) return null;
	const trimmed = value.trim();
	if (!trimmed) return null;
	if (trimmed.includes('/')) {
		return message;
	}
	return null;
}

export function getSeriesPartValidationError(value: string | undefined): string | null {
	return getSequenceValidationError(value, SERIES_PART_INVALID_MESSAGE);
}

export function getSubseriesPartValidationError(value: string | undefined): string | null {
	return getSequenceValidationError(value, SUBSERIES_PART_INVALID_MESSAGE);
}
