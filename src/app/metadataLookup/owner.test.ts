import { afterEach, expect, it, vi } from 'vitest';
import { createFakeEngine } from '../../test/fixtures/fakeEngine';
import type { SessionOutcome } from '../../types/session';
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
