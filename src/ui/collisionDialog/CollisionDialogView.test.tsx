import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, fireEvent, render, screen } from '@solidjs/testing-library';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { AppRuntimeProvider, createAppRuntime, type AppRuntime } from '../../app/runtime';
import type { PlannedOutput } from '../../types/audio';
import { CollisionDialogView } from './CollisionDialogView';

const offer = {
	titleId: 'beta',
	operationId: 'op-1',
	revision: 3,
	from: '/a/Old.m4b',
	to: '/a/New.m4b',
};
const existingOutput: PlannedOutput = {
	inputIndex: 0,
	inputPath: '/books/b.m4b',
	kind: 'final',
	requestedPath: '/out/b.m4b',
	resolvedPath: '/out/b.m4b',
	collision: { kind: 'existing_file', conflictingPath: '/out/b.m4b', detail: 'exists' },
	action: 'review_required',
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
			state.output.collisionReview = { reviewId: 7, outputs: [existingOutput] };
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

	it('keeps an answer bound to the review it was shown with and drops a repeat press', async () => {
		const engine = createFakeEngine();
		seed(engine, 'collision');
		await mount(engine);
		expect(screen.getByRole('dialog')).toHaveTextContent('1 file with the same name');
		fireEvent.click(screen.getByRole('button', { name: 'Overwrite Existing' }));
		await vi.waitFor(() => expect(engine.sessionIntents).toHaveLength(1));
		expect(screen.getByRole('dialog')).toHaveTextContent('b.m4b');

		engine.change((state) => {
			state.output.collisionReview = {
				reviewId: 8,
				outputs: [existingOutput, { ...existingOutput, resolvedPath: '/out/c.m4b' }],
			};
		});
		await vi.waitFor(() => expect(screen.getByRole('dialog')).toHaveTextContent('2 files'));
		fireEvent.click(screen.getByRole('button', { name: 'Overwrite Existing' }), { detail: 2 });
		fireEvent.click(screen.getByRole('button', { name: 'Skip Existing' }));
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toEqual([
				{ kind: 'chooseCollisionPolicy', reviewId: 7, policy: 'replace_existing' },
				{ kind: 'chooseCollisionPolicy', reviewId: 8, policy: 'skip_existing' },
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

	it('replaces a settled restart question only when the engine supplies the next one', async () => {
		const engine = createFakeEngine();
		seed(engine, 'restart');
		await mount(engine);
		engine.change((state) => {
			state.output.restartPrompt = null;
		});
		await vi.waitFor(() =>
			expect(
				screen.queryByRole('dialog', { name: 'Restart this export?' }),
			).not.toBeInTheDocument(),
		);
		engine.change((state) => {
			state.output.restartPrompt = { ...offer, titleId: 'alpha', revision: 4, to: '/a/Next.m4b' };
		});
		await vi.waitFor(() =>
			expect(screen.getByRole('dialog', { name: 'Restart this export?' })).toHaveTextContent(
				'/a/Next.m4b',
			),
		);
		fireEvent.click(screen.getByRole('button', { name: 'Restart' }));
		await vi.waitFor(() =>
			expect(engine.sessionIntents).toEqual([
				{ kind: 'restartTitle', titleId: 'alpha', revision: 4 },
			]),
		);
	});

	it.each(['collision', 'restart'] as const)(
		'Escape answers only the identified %s question',
		async (kind) => {
			const engine = createFakeEngine();
			seed(engine, kind);
			await mount(engine);
			const title =
				kind === 'collision' ? 'Resolve Existing File Conflicts' : 'Restart this export?';
			await vi.waitFor(() => expect(screen.getByRole('dialog', { name: title })).toBeVisible());
			fireEvent.keyDown(document, { key: 'Escape' });
			await vi.waitFor(() =>
				expect(engine.sessionIntents).toEqual([
					kind === 'collision'
						? { kind: 'cancelCollisionReview', reviewId: 7 }
						: { kind: 'keepTitleLocation', titleId: 'beta', revision: 3 },
				]),
			);
		},
	);
});
