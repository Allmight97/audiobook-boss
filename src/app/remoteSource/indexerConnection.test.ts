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
	runtime = await createAppRuntime({ engine });
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
it('keeps the reason for a refused edit after Test and Save are refused for it', async () => {
	const owner = await open();
	const reason = {
		code: 'invalid_input',
		category: 'validation',
		message: 'The Indexer API key has a character that cannot be sent in a request. Copy it again.',
		detail: null,
	} as const;
	engine.change((state) => {
		state.remote.connection.draftError = reason;
	});
	engine.respond = () => ({
		kind: 'rejected',
		error: {
			code: 'invalid_input',
			category: 'validation',
			message: 'Correct the Indexer connection details before saving or testing.',
			detail: null,
		},
	});

	await owner.testIndexerConnection();

	expect(owner.indexerConnection()).toMatchObject({
		saveState: 'error',
		saveError: reason.message,
	});

	engine.change((state) => {
		state.remote.connection.draftError = null;
	});
	expect(owner.indexerConnection().saveError).toBe(
		'Correct the Indexer connection details before saving or testing.',
	);
});
it('reopens from the engine draft without exposing another host’s entered key', async () => {
	await open();
	engine.change((state) => {
		state.remote.connection.apiKeyEntered = true;
		state.remote.connection.baseUrl = 'https://unsaved.test';
	});
	runtime!.dispose();
	runtime = await createAppRuntime({ engine });
	expect(runtime.remoteSource.indexerConnection()).toMatchObject({
		baseUrlDraft: 'https://unsaved.test',
		apiKeyDraft: '',
	});
});
it('shows exactly what was typed in the URL field while the engine replies, then the engine draft', async () => {
	const owner = await open();
	let answer: () => void = () => {};
	engine.respond = () =>
		new Promise((resolve) => {
			answer = () => resolve({ kind: 'applied' });
		});

	owner.patchIndexerConnectionSettings({ baseUrlDraft: 'HTTPS://Host:443/' });
	expect(owner.indexerConnection().baseUrlDraft).toBe('HTTPS://Host:443/');
	owner.patchIndexerConnectionSettings({ baseUrlDraft: 'HTTPS://Host:443/books' });
	expect(owner.indexerConnection().baseUrlDraft).toBe('HTTPS://Host:443/books');

	engine.change((state) => {
		state.remote.connection.baseUrl = 'HTTPS://Host:443/books';
	});
	answer();
	await vi.waitFor(() =>
		expect(owner.indexerConnection().baseUrlDraft).toBe('HTTPS://Host:443/books'),
	);
});
it('keeps a refused URL visible beside its reason', async () => {
	const owner = await open();
	owner.patchIndexerConnectionSettings({ baseUrlDraft: 'ftp://indexer.test' });
	engine.change((state) => {
		state.remote.connection.baseUrl = 'ftp://indexer.test';
		state.remote.connection.draftError = {
			code: 'invalid_input',
			category: 'validation',
			message: 'Indexer URL must use http or https.',
			detail: null,
		};
	});

	await vi.waitFor(() =>
		expect(owner.indexerConnection()).toMatchObject({
			baseUrlDraft: 'ftp://indexer.test',
			saveError: 'Indexer URL must use http or https.',
		}),
	);
});
it('shows the engine’s value once the engine has replied, even if it differs from what was typed', async () => {
	const owner = await open();

	owner.patchIndexerConnectionSettings({ baseUrlDraft: 'HTTPS://Host:443/' });
	await vi.waitFor(() =>
		expect(engine.sessionIntents).toContainEqual({
			kind: 'remote',
			intent: expect.objectContaining({ kind: 'editConnection' }),
		}),
	);
	engine.change((state) => {
		state.remote.connection.baseUrl = 'https://host';
	});

	await vi.waitFor(() => expect(owner.indexerConnection().baseUrlDraft).toBe('https://host'));
});
