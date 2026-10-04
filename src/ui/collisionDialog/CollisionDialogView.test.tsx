import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';
import { CollisionDialogView } from './CollisionDialogView';

const offer = {
	titleId: 'beta',
	operationId: 'op-1',
	revision: 3,
	from: '/a/Old.m4b',
	to: '/a/New.m4b',
};
const runtimes: AppRuntime[] = [];
afterEach(() => {
	cleanup();
	for (const runtime of runtimes.splice(0)) runtime.dispose();
});
async function mount(engine: FakeEngine) {
	const runtime = createAppRuntime({ engine });
	runtimes.push(runtime);
	await runtime.initialize();
	const view = render(() => (
		<AppRuntimeProvider runtime={runtime}>
			<CollisionDialogView />
		</AppRuntimeProvider>
	));
	return { runtime, view };
}
function seed(engine: FakeEngine, kind: 'collision' | 'restart'): void {
	engine.change((state) => {
		if (kind === 'collision') {
			state.output.collisionReview = { reviewId: 7, outputs: [], preview: false };
			state.output.submissionInProgress = true;
		} else {
			state.output.restartOffers = [{ ...offer, titleId: 'alpha' }, offer];
			state.output.restartPrompt = offer;
		}
	});
}

describe('engine decision dialogs', () => {
	it.each([
		['Restart', 'restartTitle'],
		['Keep Location', 'keepTitleLocation'],
	] as const)(
		'sends %s for the engine-selected prompt, not another retained offer',
		async (label, kind) => {
			const engine = createFakeEngine();
			seed(engine, 'restart');
			await mount(engine);
			expect(screen.getByRole('dialog', { name: 'Restart this export?' })).toHaveTextContent(
				'/a/New.m4b',
			);
			fireEvent.click(screen.getByRole('button', { name: label }));
			await vi.waitFor(() =>
				expect(engine.sessionIntents).toEqual([{ kind, titleId: 'beta', revision: 3 }]),
			);
		},
	);

	it.each([
		['Overwrite Existing', 'replace_existing'],
		['Skip Existing', 'skip_existing'],
		['Keep Existing', 'rename_new'],
	] as const)('binds %s to the collision review shown', async (label, policy) => {
		const engine = createFakeEngine();
		seed(engine, 'collision');
		await mount(engine);
		fireEvent.click(screen.getByRole('button', { name: label }));
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toEqual([
				{ kind: 'chooseCollisionPolicy', reviewId: 7, policy },
			]),
		);
	});

	it.each(['collision', 'restart'] as const)(
		'reattaches the unanswered %s question without answering on teardown',
		async (kind) => {
			const engine = createFakeEngine();
			seed(engine, kind);
			const first = await mount(engine);
			const title =
				kind === 'collision' ? 'Resolve Existing File Conflicts' : 'Restart this export?';
			expect(screen.getAllByRole('dialog', { name: title })).toHaveLength(1);
			first.view.unmount();
			first.runtime.dispose();
			await Promise.resolve();
			expect(engine.sessionIntents).toEqual([]);
			await mount(engine);
			expect(screen.getAllByRole('dialog', { name: title })).toHaveLength(1);
			expect(engine.sessionIntents).toEqual([]);
		},
	);

	it('Escape keeps the identified restart location, rather than cancelling file work', async () => {
		const engine = createFakeEngine();
		seed(engine, 'restart');
		await mount(engine);
		await vi.waitFor(() =>
			expect(screen.getByRole('dialog', { name: 'Restart this export?' })).toBeVisible(),
		);
		fireEvent.keyDown(document, { key: 'Escape' });
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toEqual([
				{ kind: 'keepTitleLocation', titleId: 'beta', revision: 3 },
			]),
		);
	});
});
