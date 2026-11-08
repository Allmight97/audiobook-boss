import { afterEach, describe, expect, it, vi } from 'vitest';
import { tauriClient } from '../../lib/tauri/client';
import { createFakeEngine } from '../../test/fixtures/fakeEngine';
import { createAppRuntime, type AppRuntime } from '../runtime';

describe('work operations owner', () => {
	const runtimes: AppRuntime[] = [];
	afterEach(() => {
		for (const runtime of runtimes.splice(0)) runtime.dispose();
		vi.restoreAllMocks();
	});

	async function attached(engine = createFakeEngine()): Promise<AppRuntime> {
		const runtime = await createAppRuntime({ engine });
		runtimes.push(runtime);
		return runtime;
	}

	it('surfaces a reveal rejection without leaving an unhandled promise', async () => {
		vi.spyOn(tauriClient, 'revealPath').mockRejectedValueOnce('file manager unavailable');
		const { workOperations } = await attached();

		await expect(
			workOperations.revealOutput({ outputPath: '/tmp/book.m4b' }),
		).resolves.toBeUndefined();

		expect(workOperations.view().error).toEqual({
			kind: 'reveal',
			message: 'Failed to show the exported file: file manager unavailable',
		});
	});

	it('clears a failed reveal when a later reveal works', async () => {
		vi.spyOn(tauriClient, 'revealPath')
			.mockRejectedValueOnce('file manager unavailable')
			.mockResolvedValueOnce(undefined);
		const { workOperations } = await attached();

		await workOperations.revealOutput({ outputPath: '/tmp/book.m4b' });
		expect(workOperations.view().error?.kind).toBe('reveal');

		await workOperations.revealOutput({ outputPath: '/tmp/book.m4b' });
		expect(workOperations.view().error).toBeNull();
	});

	it('keeps a failed cancel when a reveal works', async () => {
		vi.spyOn(tauriClient, 'revealPath').mockResolvedValue(undefined);
		const engine = createFakeEngine();
		const { workOperations } = await attached(engine);
		engine.respondCancel = () => Promise.reject(new Error('engine busy'));
		await workOperations.cancel('batch', 'first');

		await workOperations.revealOutput({ outputPath: '/tmp/book.m4b' });

		expect(workOperations.view().error).toEqual({
			kind: 'cancel',
			message: 'Failed to cancel title: engine busy',
		});
	});

	it('shows a failed cancel until a later cancel succeeds', async () => {
		const engine = createFakeEngine();
		const { workOperations } = await attached(engine);
		engine.respondCancel = () => Promise.reject(new Error('engine busy'));

		await workOperations.cancel('batch', 'first');
		expect(workOperations.view().error).toEqual({
			kind: 'cancel',
			message: 'Failed to cancel title: engine busy',
		});

		engine.respondCancel = undefined;
		await workOperations.cancel('batch');
		expect(workOperations.view().error).toBeNull();
		expect(engine.cancelledWork).toEqual([
			{ operationId: 'batch', childJobId: 'first' },
			{ operationId: 'batch', childJobId: undefined },
		]);
	});

	it('marks a whole-operation cancel pending only until its reply', async () => {
		const engine = createFakeEngine();
		const { workOperations } = await attached(engine);
		let reply: () => void = () => undefined;
		engine.respondCancel = () =>
			new Promise((resolve) => {
				reply = resolve;
			});

		const cancelling = workOperations.cancel('batch');
		expect(workOperations.view().cancelPendingByOperationId.batch).toBe(true);
		await vi.waitFor(() => expect(engine.cancelledWork).toHaveLength(1));
		reply();
		await cancelling;
		expect(workOperations.view().cancelPendingByOperationId.batch).toBeUndefined();
	});

	it('shows nothing from a cancel that fails after disposal', async () => {
		const engine = createFakeEngine();
		const runtime = await attached(engine);
		let fail: (error: Error) => void = () => undefined;
		engine.respondCancel = () =>
			new Promise((_resolve, reject) => {
				fail = reject;
			});

		const cancelling = runtime.workOperations.cancel('batch', 'first');
		await vi.waitFor(() => expect(engine.cancelledWork).toHaveLength(1));
		runtime.dispose();
		fail(new Error('late failure'));
		await cancelling;

		expect(runtime.workOperations.view().error).toBeNull();
	});
});
