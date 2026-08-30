import { describe, expect, it } from 'vitest';

import * as outputPanel from '..';

const EXPECTED_OUTPUT_PANEL_EXPORTS = [
	'OutputView',
	'applyOutputDefaultsFromSettings',
	'readOutputDefaultsFromState',
	'readOutputRequestConfig',
	'runOutputPlanReviewWorkflow',
] as const;

describe('Output Panel Runtime public API contract', () => {
	it('pins the output panel public export strip', () => {
		expect(Object.keys(outputPanel).sort()).toEqual([...EXPECTED_OUTPUT_PANEL_EXPORTS].sort());
	});
});
