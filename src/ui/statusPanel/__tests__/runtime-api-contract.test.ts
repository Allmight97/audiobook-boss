import { describe, expect, it } from 'vitest';

import * as statusPanel from '..';

const EXPECTED_STATUS_PANEL_EXPORTS = ['StatusPanelView'] as const;

describe('Status Panel public API contract', () => {
	it('pins the status panel public export strip', () => {
		expect(Object.keys(statusPanel).sort()).toEqual([...EXPECTED_STATUS_PANEL_EXPORTS].sort());
	});
});
