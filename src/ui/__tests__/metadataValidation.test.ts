import { describe, expect, it, vi } from 'vitest';

import {
	firstMetadataIntentValidationError,
	getSeriesPartValidationError,
	getSubseriesPartValidationError,
	validateMetadataDraftIntent,
	type ValidateMetadataIntentPatch,
} from '../metadataValidation';

const validResult = {
	isValid: true,
	metadataPatch: {},
	fieldErrors: [],
};

describe('metadata validation adapters', () => {
	it('reads backend-owned field errors for series and subseries', () => {
		const result = {
			isValid: false,
			metadataPatch: {},
			fieldErrors: [
				{
					field: 'series_part' as const,
					code: 'series_part_contains_slash' as const,
					message: "Series sequence (#) cannot include '/'. Use a plain number like 24.",
				},
				{
					field: 'subseries_part' as const,
					code: 'subseries_part_contains_slash' as const,
					message: "Sub-series sequence (#) cannot include '/'. Use a plain number like 24.",
				},
			],
		};

		expect(getSeriesPartValidationError(result)).toBe(
			"Series sequence (#) cannot include '/'. Use a plain number like 24.",
		);
		expect(getSubseriesPartValidationError(result)).toBe(
			"Sub-series sequence (#) cannot include '/'. Use a plain number like 24.",
		);
		expect(firstMetadataIntentValidationError(result)).toBe(
			"Series sequence (#) cannot include '/'. Use a plain number like 24.",
		);
	});

	it('builds draft intent and returns the backend-normalized patch', async () => {
		const validateMetadataIntentPatch: ValidateMetadataIntentPatch = vi.fn().mockResolvedValue({
			...validResult,
			metadataPatch: {
				date: { op: 'set', value: '2024-07' },
			},
		});

		const validation = await validateMetadataDraftIntent(
			{ date: '2024-07-15' },
			validateMetadataIntentPatch,
		);

		expect(validateMetadataIntentPatch).toHaveBeenCalledWith({
			date: { op: 'set', value: '2024-07-15' },
		});
		expect(validation.intentPatch).toEqual({
			date: { op: 'set', value: '2024-07' },
		});
	});
});
