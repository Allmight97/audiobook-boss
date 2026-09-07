import { expect, it } from 'vitest';
import * as settings from '.';

it('keeps the Settings UI public strip on views', () => {
	expect(Object.keys(settings).sort()).toEqual([
		'AppSettingsDialogView',
		'SettingsPersistenceNotice',
	]);
});
