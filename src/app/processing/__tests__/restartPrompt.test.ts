import { afterEach, describe, expect, it, vi } from 'vitest';
import { ask } from '@tauri-apps/plugin-dialog';
import { createFakeEngine } from '../../../test/fixtures/fakeEngine';
import { createAppRuntime, type AppRuntime } from '../../runtime';

describe('restart offers', () => {
	let runtime: AppRuntime | undefined;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
	});

	it('asks once per offer and keeps the location when the user declines', async () => {
		const engine = createFakeEngine();
		runtime = createAppRuntime({ engine });
		await runtime.initialize();
		vi.mocked(ask).mockResolvedValueOnce(false);

		const offer = { titleId: 'alpha', revision: 3, from: '/a/Old.m4b', to: '/a/New.m4b' };
		engine.change((state) => {
			state.output.restartOffers = [offer];
		});
		engine.change((state) => {
			state.output.restartOffers = [offer];
		});

		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'keepTitleLocation',
				titleId: 'alpha',
				revision: 3,
			}),
		);
		expect(ask).toHaveBeenCalledTimes(1);
		expect(vi.mocked(ask).mock.calls[0]?.[0]).toContain('/a/New.m4b');
	});
});
