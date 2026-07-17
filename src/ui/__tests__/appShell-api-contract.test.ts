import { describe, expect, it } from 'vitest';

describe('appShell Public API Strip', () => {
	it('exports exactly the documented surface', async () => {
		const appShell = await import('../appShell');

		expect(Object.keys(appShell).sort()).toEqual([
			'AppShellIsland',
			'applyDensityPreference',
			'applyRailWidthPreference',
			'readRailWidth',
			'setDensityFromUser',
		]);
	});
});
