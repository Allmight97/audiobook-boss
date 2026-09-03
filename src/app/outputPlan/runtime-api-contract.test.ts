import { describe, expect, it } from 'vitest';

import * as outputPlan from '.';

const EXPECTED_APP_OUTPUT_PLAN_EXPORTS = [
	'CUSTOM_TEMPLATE_PLACEHOLDER',
	'createOutputOwner',
	'estimateEncodedSizeBytes',
	'formatEstimatedSizeText',
	'runOutputPlanReviewWorkflow',
] as const;

describe('app Output Plan Public API Strip', () => {
	it('pins the app outputPlan public export strip', () => {
		expect(Object.keys(outputPlan).sort()).toEqual([...EXPECTED_APP_OUTPUT_PLAN_EXPORTS].sort());
	});

	it('does not export last-writer bind or compatibility getters', () => {
		expect(outputPlan).not.toHaveProperty('bindOutputOwner');
		expect(outputPlan).not.toHaveProperty('boundOutputOwner');
		expect(outputPlan).not.toHaveProperty('readOutputRequestConfig');
		expect(outputPlan).not.toHaveProperty('applyOutputDefaultsFromSettings');
		expect(outputPlan).not.toHaveProperty('readOutputDefaultsFromState');
		expect(outputPlan).not.toHaveProperty('resetOutputPlan');
		expect(outputPlan).not.toHaveProperty('resetOutputPlanTimers');
	});
});
