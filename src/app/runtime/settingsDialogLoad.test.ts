import { expect, it, vi } from 'vitest';
import { createFakeEngine } from '../../test/fixtures/fakeEngine';
import { createAppRuntime } from './index';

const loadConnection = { kind: 'remote', intent: { kind: 'loadConnection' } };

it('loads the Indexer connection when the Settings dialog opens, once per opening', async () => {
	const engine = createFakeEngine();
	const runtime = await createAppRuntime({ engine });
	expect(engine.sessionIntents).not.toContainEqual(loadConnection);

	runtime.settings.openDialog();
	await vi.waitFor(() =>
		expect(engine.sessionIntents.filter((intent) => intent.kind === 'remote')).toEqual([
			loadConnection,
		]),
	);

	runtime.settings.closeDialog();
	await vi.waitFor(() => expect(runtime.settings.dialog().isOpen).toBe(false));
	runtime.settings.openDialog();
	await vi.waitFor(() =>
		expect(engine.sessionIntents.filter((intent) => intent.kind === 'remote')).toEqual([
			loadConnection,
			loadConnection,
		]),
	);
	runtime.dispose();
});
