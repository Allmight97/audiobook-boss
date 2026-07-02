import { describe, expect, it, vi } from 'vitest';

import { validateMetadataDraft, type ValidateMetadataIntentPatch } from '../validation';

const validResult = {
	isValid: true,
	metadataPatch: {},
	fieldErrors: [],
};

describe('metadata draft validation outcome', () => {
	it('reshapes backend-owned field errors into first/byField outcomes', async () => {
		const validate: ValidateMetadataIntentPatch = vi.fn().mockResolvedValue({
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
		});

		const validation = await validateMetadataDraft({ series_part: '2/4' }, validate);

		expect(validation.ok).toBe(false);
		expect(validation.errors.first).toBe(
			"Series sequence (#) cannot include '/'. Use a plain number like 24.",
		);
		expect(validation.errors.byField.series_part).toBe(
			"Series sequence (#) cannot include '/'. Use a plain number like 24.",
		);
		expect(validation.errors.byField.subseries_part).toBe(
			"Sub-series sequence (#) cannot include '/'. Use a plain number like 24.",
		);
	});

	it('builds draft intent and returns the backend-normalized patch', async () => {
		const validate: ValidateMetadataIntentPatch = vi.fn().mockResolvedValue({
			...validResult,
			metadataPatch: {
				date: { op: 'set', value: '2024-07' },
			},
		});

		const validation = await validateMetadataDraft({ date: '2024-07-15' }, validate);

		expect(validate).toHaveBeenCalledWith({
			date: { op: 'set', value: '2024-07-15' },
		});
		expect(validation.ok).toBe(true);
		expect(validation.errors.first).toBeNull();
		expect(validation.intentPatch).toEqual({
			date: { op: 'set', value: '2024-07' },
		});
	});
});
