import { describe, expect, it } from 'vitest';

import * as workCenter from '..';

const EXPECTED_WORK_CENTER_EXPORTS = [
	'WorkCenterView',
	'initializeWorkCenter',
	'workCenterState',
] as const;

describe('Work Center public API contract', () => {
	it('pins the work center public export strip', () => {
		expect(Object.keys(workCenter).sort()).toEqual([...EXPECTED_WORK_CENTER_EXPORTS].sort());
	});
});
