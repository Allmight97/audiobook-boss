import { afterEach, expect, it, vi } from 'vitest';
import { createFakeEngine, type FakeEngine } from '../../test/fixtures/fakeEngine';
import { createAppRuntime, type AppRuntime } from '../runtime';

let runtime: AppRuntime | undefined;
let engine: FakeEngine;
afterEach(() => runtime?.dispose());
async function open() {
	engine = createFakeEngine();
	engine.change((state) => {
		state.remote.connection = {
			...state.remote.connection,
			baseUrl: 'https://indexer.test',
			categoryIds: [3030],
			apiKeyConfigured: true,
		};
	});
	runtime = createAppRuntime({ engine });
	await runtime.initialize();
	return runtime.remoteSource;
}
it('sends edited draft, Test, and Save as separate intents and never reads the saved key', async () => {
	const owner = await open();
	await owner.loadIndexerConnectionSettings();
	owner.patchIndexerConnectionSettings({
		baseUrlDraft: 'https://next.test',
		apiKeyDraft: 'entered-key',
		categoryIdsDraft: [3030, 3000],
	});
	await vi.waitFor(() =>
		expect(engine.sessionIntents).toContainEqual({
			kind: 'remote',
			intent: {
				kind: 'editConnection',
				baseUrl: 'https://next.test',
				apiKey: 'entered-key',
				categoryIds: [3030, 3000],
			},
		}),
	);
	await owner.testIndexerConnection();
	expect(engine.sessionIntents).toContainEqual({
		kind: 'remote',
		intent: { kind: 'testConnection' },
	});
	expect(engine.sessionIntents).not.toContainEqual({
		kind: 'remote',
		intent: { kind: 'saveConnection' },
	});
	engine.respond = () => ({ kind: 'remoteSaved' });
	await owner.saveIndexerConnectionSettings();
	expect(engine.sessionIntents).toContainEqual({
		kind: 'remote',
		intent: { kind: 'saveConnection' },
	});
});
it('renders engine draft status and preserves the entered key until the engine confirms it was cleared', async () => {
	const owner = await open();
	owner.patchIndexerConnectionSettings({ apiKeyDraft: 'new-key' });
	engine.change((state) => {
		state.remote.connection.apiKeyEntered = true;
		state.remote.connection.save = { kind: 'running' };
		state.remote.connection.test = { kind: 'succeeded' };
		state.remote.connection.testResult = { ok: false, message: 'Key rejected' };
	});
	await vi.waitFor(() => expect(owner.indexerConnection().saveState).toBe('saving'));
	expect(owner.indexerConnection().testMessage).toBe('Key rejected');
	expect(owner.indexerConnection().testState).toBe('error');
	expect(owner.indexerConnection().apiKeyDraft).toBe('new-key');
	engine.change((state) => {
		state.remote.connection.apiKeyEntered = false;
		state.remote.connection.save = { kind: 'succeeded' };
	});
	await vi.waitFor(() => expect(owner.indexerConnection().apiKeyDraft).toBe(''));
	expect(owner.indexerConnection().saveState).toBe('saved');
	expect(owner.indexerConnection().apiKeyConfigured).toBe(true);
});
it('reopens from the engine draft without exposing another host’s entered key', async () => {
	await open();
	engine.change((state) => {
		state.remote.connection.apiKeyEntered = true;
		state.remote.connection.baseUrl = 'https://unsaved.test';
	});
	runtime!.dispose();
	runtime = createAppRuntime({ engine });
	await runtime.initialize();
	expect(runtime.remoteSource.indexerConnection()).toMatchObject({
		baseUrlDraft: 'https://unsaved.test',
		apiKeyDraft: '',
	});
});
