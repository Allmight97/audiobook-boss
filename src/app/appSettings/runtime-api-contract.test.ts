import { expect, it } from 'vitest';
import * as settings from '.';

it('keeps the Settings public strip on owner creation', () => {
	expect(Object.keys(settings).sort()).toEqual(['createSettingsOwner']);
});
