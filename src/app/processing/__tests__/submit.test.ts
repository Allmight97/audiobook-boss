import { afterEach, describe, expect, it, vi } from 'vitest';
import { createAppRuntime, type AppRuntime } from '../../runtime';
import { createFakeEngine } from '../../../test/fixtures/fakeEngine';

describe('submission snapshot adapter', () => {
	const runtimes: AppRuntime[] = [];
	afterEach(() => {
		for (const runtime of runtimes.splice(0)) runtime.dispose();
	});

	it('renders outcomes received without a local waiter and trusts engine busy facts', async () => {
		const engine = createFakeEngine();
		const app = createAppRuntime({ engine });
		runtimes.push(app);
		await app.initialize();
		engine.change((state) => {
			state.output.submission = { kind: 'submitted', operationId: 'op-1', title: 'Alpha' };
		});
		await vi.waitFor(() =>
			expect(app.processing.status().stepText).toBe('Current Step: Submitted to Work Center.'),
		);
		engine.change((state) => {
			state.output.submission = { kind: 'refused', reason: { kind: 'noOutputDirectory' } };
			state.output.submissionInProgress = true;
		});
		await vi.waitFor(() =>
			expect(app.processing.status().stepText).toContain('Choose an output folder'),
		);
		expect(app.processing.isProcessing()).toBe(true);
		expect(app.settings.concurrency().controlsEnabled).toBe(false);
		engine.change((state) => {
			state.output.submissionInProgress = false;
		});
		await vi.waitFor(() => expect(app.settings.concurrency().controlsEnabled).toBe(true));
	});

	it('sends export, preview and targeted restart intents without building or continuing work', async () => {
		const engine = createFakeEngine();
		const app = createAppRuntime({ engine });
		runtimes.push(app);
		await app.initialize();
		await app.processing.start();
		await app.processing.start({ previewSeconds: 30 });
		await app.processing.restart({
			titleId: 'alpha',
			operationId: 'op-1',
			revision: 3,
			from: '/old',
			to: '/new',
		});
		expect(engine.sessionIntents).toEqual([
			{ kind: 'submit' },
			{ kind: 'preview', seconds: 30 },
			{ kind: 'restartTitle', titleId: 'alpha', revision: 3 },
		]);
	});

	it('words backend cancellation without reporting failure', async () => {
		const engine = createFakeEngine();
		const app = createAppRuntime({ engine });
		runtimes.push(app);
		await app.initialize();
		engine.change((state) => {
			state.output.submission = {
				kind: 'failed',
				error: {
					code: 'processing_cancelled',
					category: 'cancellation',
					message: 'Processing was cancelled.',
					detail: null,
				},
			};
		});
		await vi.waitFor(() => expect(app.processing.status().stepText).toBe('Preview was cancelled.'));
	});
});
