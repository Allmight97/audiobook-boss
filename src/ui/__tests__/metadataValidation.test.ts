import { describe, expect, it } from 'vitest';

import {
	getSeriesPartValidationError,
	getSubseriesPartValidationError,
} from '../metadataValidation';

describe('metadata validation', () => {
	it('accepts empty and plain sequence values', () => {
		expect(getSeriesPartValidationError(undefined)).toBeNull();
		expect(getSeriesPartValidationError('')).toBeNull();
		expect(getSeriesPartValidationError(' 24 ')).toBeNull();
		expect(getSubseriesPartValidationError('3.5')).toBeNull();
	});

	it('preserves series and subseries validation messages', () => {
		expect(getSeriesPartValidationError('1/2')).toBe(
			"Series sequence (#) cannot include '/'. Use a plain number like 24.",
		);
		expect(getSubseriesPartValidationError('1/2')).toBe(
			"Sub-series sequence (#) cannot include '/'. Use a plain number like 24.",
		);
	});
});
