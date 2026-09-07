import { expect, it } from 'vitest';
import * as operations from '.';

it('keeps the Work Operations public strip on the owner factory', () => {
	expect(Object.keys(operations)).toEqual(['createWorkOperationsOwner']);
});
