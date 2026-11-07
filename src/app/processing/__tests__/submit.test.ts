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
		const app = await createAppRuntime({ engine });
		runtimes.push(app);
		engine.change((state) => {
			state.output.submission = {
				kind: 'submitted',
				operationId: 'op-1',
				title: 'Alpha',
				leftOut: 0,
			};
		});
		await vi.waitFor(() =>
			expect(app.processing.status().stepText).toBe('Current Step: Submitted to Work Center.'),
		);
		expect(app.processing.status().statusText).toBe('Idle');
		expect(app.processing.status().progressPercentage).toBe(0);
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

	it('says how many invalid files a Submit left out', async () => {
		const engine = createFakeEngine();
		const app = await createAppRuntime({ engine });
		runtimes.push(app);
		const submitted = (leftOut: number) =>
			engine.change((state) => {
				state.output.submission = {
					kind: 'submitted',
					operationId: `op-${leftOut}`,
					title: 'Alpha',
					leftOut,
				};
			});
		submitted(1);
		await vi.waitFor(() =>
			expect(app.processing.status().stepText).toBe(
				'Current Step: Submitted to Work Center. 1 invalid file was left out.',
			),
		);
		submitted(2);
		await vi.waitFor(() =>
			expect(app.processing.status().stepText).toBe(
				'Current Step: Submitted to Work Center. 2 invalid files were left out.',
			),
		);
	});

	it('shows work that finished before its restart as completed', async () => {
		const engine = createFakeEngine();
		const app = await createAppRuntime({ engine });
		runtimes.push(app);
		engine.change((state) => {
			state.output.submission = {
				kind: 'finishedBeforeRestart',
				outputs: { updated: 1, elsewhere: 0, restartOffered: 0, failed: 0 },
			};
		});
		await vi.waitFor(() => expect(app.processing.status().statusText).toBe('Completed'));
		expect(app.processing.status().progressPercentage).toBe(100);
	});

	it('sends export, preview and targeted restart intents without building or continuing work', async () => {
		const engine = createFakeEngine();
		const app = await createAppRuntime({ engine });
		runtimes.push(app);
		await app.processing.start();
		await app.processing.preview();
		await app.processing.restart({
			titleId: 'alpha',
			operationId: 'op-1',
			revision: 3,
			from: '/old',
			to: '/new',
		});
		expect(engine.sessionIntents).toEqual([
			{ kind: 'submit' },
			{ kind: 'preview' },
			{ kind: 'restartTitle', titleId: 'alpha', revision: 3 },
		]);
	});

	it('drops a failed send from the status line once the engine reports a new submission', async () => {
		const engine = createFakeEngine();
		engine.respond = (intent) =>
			intent.kind === 'submit'
				? {
						kind: 'rejected',
						error: { code: 'io_error', category: 'io', message: 'Disk full', detail: null },
					}
				: undefined;
		const app = await createAppRuntime({ engine });
		runtimes.push(app);
		await app.processing.start();
		expect(app.processing.status().stepText).toBe('Error: Processing failed: Disk full');
		expect(app.processing.status().stepTone).toBe('error');
		engine.change((state) => {
			state.output.submission = { kind: 'preparing', preview: false };
		});
		await vi.waitFor(() => expect(app.processing.status().stepText).toBe('Current Step: Ready'));
		expect(app.processing.status().stepTone).toBe('normal');
	});
});
