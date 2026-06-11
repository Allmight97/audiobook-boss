import { beforeEach, describe, expect, it } from 'vitest';

import * as outputPanel from '..';

const EXPECTED_OUTPUT_PANEL_EXPORTS = [
	'OutputPanelIsland',
	'applyOutputDefaultsFromSettings',
	'initOutputPanel',
	'readEstimatedSizeText',
	'readOutputDefaultsFromState',
	'readOutputRequestConfig',
	'runOutputPlanReviewWorkflow',
	'updateEstimatedSize',
	'updateOutputPath',
] as const;

describe('Output Panel Runtime public API contract', () => {
	beforeEach(() => {
		outputPanel.initOutputPanel();
	});

	it('pins the output panel public export strip', () => {
		expect(Object.keys(outputPanel).sort()).toEqual([...EXPECTED_OUTPUT_PANEL_EXPORTS].sort());
	});

	it('reads estimated size through the public accessor after refresh', () => {
		outputPanel.updateEstimatedSize();
		expect(outputPanel.readEstimatedSizeText()).toMatch(/^~/);
	});
});
