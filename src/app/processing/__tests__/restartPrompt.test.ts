import { afterEach, describe, expect, it, vi } from 'vitest';
import { ask } from '@tauri-apps/plugin-dialog';
import { createFakeEngine, type FakeEngine } from '../../../test/fixtures/fakeEngine';
import { createAppRuntime, type AppRuntime } from '../../runtime';

const offer = {
	titleId: 'alpha',
	operationId: 'op-1',
	revision: 3,
	from: '/a/Old.m4b',
	to: '/a/New.m4b',
};

describe('restart offers', () => {
	let runtime: AppRuntime | undefined;
	let engine: FakeEngine;

	afterEach(() => {
		runtime?.dispose();
		runtime = undefined;
		vi.mocked(ask).mockReset();
	});

	async function open(answer: boolean): Promise<void> {
		engine = createFakeEngine();
		runtime = createAppRuntime({ engine });
		await runtime.initialize();
		vi.mocked(ask).mockResolvedValue(answer);
		engine.change((state) => {
			state.output.restartOffers = [offer];
		});
		await vi.waitFor(() => expect(ask).toHaveBeenCalledTimes(1));
	}

	it('asks once per offer and keeps the location when the user declines', async () => {
		await open(false);
		expect(vi.mocked(ask).mock.calls[0]?.[0]).toContain('/a/New.m4b');

		// The same offer again, after it was asked, is not asked again.
		engine.change((state) => {
			state.output.restartOffers = [{ ...offer }];
		});
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'keepTitleLocation',
				titleId: 'alpha',
				revision: 3,
			}),
		);
		expect(ask).toHaveBeenCalledTimes(1);
	});

	it('restarts the title when the user confirms', async () => {
		await open(true);
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toContainEqual({
				kind: 'restartTitle',
				titleId: 'alpha',
				revision: 3,
			}),
		);
	});
});
