import { beforeEach, describe, expect, it } from 'vitest';

import * as statusPanel from '..';

const EXPECTED_STATUS_PANEL_EXPORTS = [
	'StatusPanelIsland',
	'initStatusPanel',
	'isStatusPanelProcessing',
	'pushStatusPanelTransientStatus',
	'triggerCancelAllFromStatusPanel',
	'triggerProcessFromStatusPanel',
	'updateStatusPanelConcurrencyStatus',
] as const;

describe('Status Panel Runtime public API contract', () => {
	beforeEach(() => {
		statusPanel.updateStatusPanelConcurrencyStatus('');
	});

	it('pins the status panel public export strip', () => {
		expect(Object.keys(statusPanel).sort()).toEqual([...EXPECTED_STATUS_PANEL_EXPORTS].sort());
	});
});
