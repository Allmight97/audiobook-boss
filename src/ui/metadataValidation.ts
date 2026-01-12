/**
 * Metadata validation helpers for UI workflows.
 */

const SERIES_PART_INVALID_MESSAGE =
  "Series # cannot include '/'. Use a plain number like 24.";

export function getSeriesPartValidationError(
  value: string | undefined
): string | null {
  if (!value) return null;
  const trimmed = value.trim();
  if (!trimmed) return null;
  if (trimmed.includes("/")) {
    return SERIES_PART_INVALID_MESSAGE;
  }
  return null;
}
