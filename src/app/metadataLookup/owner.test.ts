import { afterEach, expect, it, vi } from 'vitest';
import { createFakeEngine } from '../../test/fixtures/fakeEngine';
import type { SessionIntent, SessionOutcome } from '../../types/session';
import { createAppRuntime, type AppRuntime } from '../runtime';
let runtime: AppRuntime | undefined;
afterEach(() => runtime?.dispose());
it('drops delayed query echoes when lookup advances and preserves newer typing', async () => {
	const engine = createFakeEngine();
	engine.change((state) => {
		state.lookup.open = true;
		state.lookup.queuePosition = { index: 0, total: 2, path: '/books/alpha.m4b' };
		state.lookup.titleQuery = 'Alpha';
		state.lookup.authorQuery = 'First Author';
	});
	const replies: Array<(outcome: SessionOutcome) => void> = [];
	engine.respond = (intent) =>
		intent.kind === 'lookupSetTitleQuery' || intent.kind === 'lookupSetAuthorQuery'
			? new Promise<SessionOutcome>((resolve) => {
					replies.push(resolve);
				})
			: undefined;
	runtime = createAppRuntime({ engine });
	await runtime.initialize();
	runtime.lookup.setTitleQuery('Old title draft');
	runtime.lookup.setAuthorQuery('Old author draft');
	expect(runtime.lookup.view().titleQuery).toBe('Old title draft');
	await vi.waitFor(() => expect(replies).toHaveLength(2));
	engine.change((state) => {
		state.lookup.queuePosition = { index: 1, total: 2, path: '/books/beta.m4b' };
		state.lookup.titleQuery = 'Beta';
		state.lookup.authorQuery = 'Second Author';
	});
	expect(runtime.lookup.view().titleQuery).toBe('Beta');
	expect(runtime.lookup.view().authorQuery).toBe('Second Author');
	const finishOldAuthor = replies[1];
	runtime.lookup.setAuthorQuery('New author draft');
	await vi.waitFor(() => expect(replies).toHaveLength(3));
	finishOldAuthor({ kind: 'applied' });
	await Promise.resolve();
	expect(runtime.lookup.view().authorQuery).toBe('New author draft');
});

function refusing(
	kind: SessionIntent['kind'],
): (intent: SessionIntent) => SessionOutcome | undefined {
	return (intent) =>
		intent.kind === kind
			? {
					kind: 'rejected',
					error: {
						code: 'invalid_input',
						category: 'validation',
						message: 'Lookup moved on.',
						detail: null,
					},
				}
			: undefined;
}

it('shows a refused query and drops its typed text', async () => {
	const engine = createFakeEngine();
	engine.change((state) => {
		state.lookup.open = true;
		state.lookup.titleQuery = 'Alpha';
	});
	engine.respond = refusing('lookupSetTitleQuery');
	runtime = createAppRuntime({ engine });
	await runtime.initialize();

	runtime.lookup.setTitleQuery('Typed');
	expect(runtime.lookup.view().titleQuery).toBe('Typed');

	await vi.waitFor(() => expect(runtime?.engine.refusal()).toBe('Lookup moved on.'));
	expect(runtime.lookup.view().titleQuery).toBe('Alpha');
});

it('moves focus to the form only after the engine closes the lookup for manual entry', async () => {
	const engine = createFakeEngine();
	engine.change((state) => {
		state.lookup.open = true;
	});
	const input = document.createElement('input');
	input.id = 'meta-title';
	document.body.append(input);
	let release: () => void = () => {};
	engine.respond = (intent) =>
		intent.kind === 'lookupClose'
			? new Promise<undefined>((resolve) => {
					release = () => resolve(undefined);
				})
			: undefined;
	runtime = createAppRuntime({ engine });
	await runtime.initialize();

	const ran = runtime.lookup.run({ type: 'manualEntry' });
	await vi.waitFor(() =>
		expect(engine.sessionIntents.map((intent) => intent.kind)).toContain('lookupClose'),
	);
	await Promise.resolve();
	expect(document.activeElement).not.toBe(input);

	release();
	await ran;
	await vi.waitFor(() => expect(document.activeElement).toBe(input));
	input.remove();
});

it('shows a refused manual entry and leaves focus where it was', async () => {
	const engine = createFakeEngine();
	engine.change((state) => {
		state.lookup.open = true;
	});
	const input = document.createElement('input');
	input.id = 'meta-title';
	document.body.append(input);
	engine.respond = refusing('lookupClose');
	runtime = createAppRuntime({ engine });
	await runtime.initialize();

	await runtime.lookup.run({ type: 'manualEntry' });
	await Promise.resolve();

	expect(runtime.engine.refusal()).toBe('Lookup moved on.');
	expect(document.activeElement).not.toBe(input);
	input.remove();
});
