import { describe, expect, it } from 'vitest';
import * as operationsBar from '..';

describe('Operations Bar Runtime public API contract', () => {
	it('pins the operations bar public export strip', () => {
		expect(Object.keys(operationsBar).sort()).toEqual(['OperationsBarIsland']);
	});
});
