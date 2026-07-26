import { describe, expect, it } from 'vitest';
import * as workCenter from '..';

describe('Work Center Runtime public API contract', () => {
	it('pins the work center public export strip', () => {
		expect(Object.keys(workCenter).sort()).toEqual([
			'WorkCenterIsland',
			'deriveWorkOperationCounts',
			'initializeWorkCenter',
			'readWorkActivityByInputId',
			'workCenterState',
		]);
	});
});
