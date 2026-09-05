import { afterEach, describe, expect, it, vi } from 'vitest';
import { createTestAppRuntime } from '../runtime/harness';
import type { AppRuntime } from '../runtime';
import { tauriClient } from '../../lib/tauri/client';
import type { RemoteIndexerConnection } from '../../types/remoteSource';

let runtime: AppRuntime | undefined;
afterEach(() => {
	runtime?.dispose();
	runtime = undefined;
	vi.restoreAllMocks();
});
const configured: RemoteIndexerConnection = {
	baseUrl: 'http://indexer.test',
	categoryIds: [3030],
	apiKeyConfigured: true,
};

describe('Indexer connection owner intents', () => {
	it('tests the edited connection without saving and clears the write-only key after Save', async () => {
		vi.spyOn(tauriClient, 'getRemoteSourceIndexerConnection').mockResolvedValue(configured);
		const save = vi
			.spyOn(tauriClient, 'updateRemoteSourceIndexerConnection')
			.mockResolvedValue(configured);
		const test = vi
			.spyOn(tauriClient, 'testRemoteSourceIndexerConnection')
			.mockResolvedValue({ ok: false, message: 'API key rejected' });
		runtime = createTestAppRuntime();
		const owner = runtime.remoteSource;
		await owner.loadIndexerConnectionSettings();
		expect(owner.indexerConnection().apiKeyDraft).toBe('');
		owner.patchIndexerConnectionSettings({
			baseUrlDraft: ' http://indexer.test ',
			apiKeyDraft: ' new-key ',
			categoryIdsDraft: [3030, 3000],
		});
		await owner.testIndexerConnection();
		expect(test).toHaveBeenCalledExactlyOnceWith({
			baseUrl: 'http://indexer.test',
			apiKey: 'new-key',
			categoryIds: [3030, 3000],
		});
		expect(save).not.toHaveBeenCalled();
		expect(owner.indexerConnection().testMessage).toBe('API key rejected');
		expect(owner.indexerConnection().testState).toBe('error');
		await owner.saveIndexerConnectionSettings();
		expect(owner.indexerConnection().apiKeyDraft).toBe('');
		expect(owner.indexerConnection().apiKeyConfigured).toBe(true);
		await owner.saveIndexerConnectionSettings();
		expect(save).toHaveBeenLastCalledWith({ baseUrl: 'http://indexer.test', categoryIds: [3030] });
	});

	it('refreshes an already-open Indexer account after accepted Save', async () => {
		vi.spyOn(tauriClient, 'updateRemoteSourceIndexerConnection').mockResolvedValue(configured);
		const account = vi
			.spyOn(tauriClient, 'getRemoteSourceAccountState')
			.mockResolvedValue({ providerId: 'indexer', status: 'connected' });
		runtime = createTestAppRuntime();
		const owner = runtime.remoteSource;
		vi.spyOn(tauriClient, 'listRemoteSourceProviders').mockResolvedValue([]);
		vi.mocked(tauriClient.getRemoteSourceAccountState).mockResolvedValueOnce({
			providerId: 'indexer',
			status: 'needsAuth',
		});
		await owner.open({ lane: 'indexer' });
		expect(owner.view().accountState?.status).toBe('needsAuth');
		owner.patchIndexerConnectionSettings({ baseUrlDraft: configured.baseUrl!, apiKeyDraft: 'key' });
		await owner.saveIndexerConnectionSettings();
		expect(account).toHaveBeenCalledWith('indexer');
		expect(owner.view().accountState?.status).toBe('connected');
	});

	it('reports a failed post-save refresh while retaining successful persistence', async () => {
		vi.spyOn(tauriClient, 'updateRemoteSourceIndexerConnection').mockResolvedValue(configured);
		vi.spyOn(tauriClient, 'listRemoteSourceProviders').mockResolvedValue([]);
		vi.spyOn(tauriClient, 'getRemoteSourceAccountState')
			.mockResolvedValueOnce({ providerId: 'indexer', status: 'needsAuth' })
			.mockRejectedValueOnce(new Error('Account refresh unavailable'));
		runtime = createTestAppRuntime();
		const owner = runtime.remoteSource;
		await owner.open({ lane: 'indexer' });
		owner.patchIndexerConnectionSettings({ baseUrlDraft: configured.baseUrl!, apiKeyDraft: 'key' });
		await expect(owner.saveIndexerConnectionSettings()).resolves.toBeUndefined();
		expect(owner.indexerConnection().saveState).toBe('saved');
		expect(owner.indexerConnection().apiKeyDraft).toBe('');
		expect(owner.view().statusMessage).toContain('Connection saved, but account refresh failed.');
	});

	it('does not let initial loading overwrite edits made before the load returns', async () => {
		let finish!: (connection: RemoteIndexerConnection) => void;
		vi.spyOn(tauriClient, 'getRemoteSourceIndexerConnection').mockReturnValue(
			new Promise((resolve) => {
				finish = resolve;
			}),
		);
		runtime = createTestAppRuntime();
		const owner = runtime.remoteSource;
		const loading = owner.loadIndexerConnectionSettings();
		owner.patchIndexerConnectionSettings({
			apiKeyDraft: 'typed-while-loading',
			baseUrlDraft: 'http://new.test',
		});
		finish(configured);
		await loading;
		expect(owner.indexerConnection().apiKeyDraft).toBe('typed-while-loading');
		expect(owner.indexerConnection().baseUrlDraft).toBe('http://new.test');
	});
	it('clears pending Test status when Save supersedes that draft test', async () => {
		let finish!: (value: { ok: boolean; message: string }) => void;
		vi.spyOn(tauriClient, 'testRemoteSourceIndexerConnection').mockReturnValue(
			new Promise((resolve) => {
				finish = resolve;
			}),
		);
		vi.spyOn(tauriClient, 'updateRemoteSourceIndexerConnection').mockResolvedValue(configured);
		runtime = createTestAppRuntime();
		const owner = runtime.remoteSource;
		owner.patchIndexerConnectionSettings({ baseUrlDraft: configured.baseUrl!, apiKeyDraft: 'key' });
		const testing = owner.testIndexerConnection();
		await owner.saveIndexerConnectionSettings();
		finish({ ok: true, message: 'Old test' });
		await testing;
		expect(owner.indexerConnection().testState).toBe('idle');
		expect(owner.indexerConnection().testMessage).toBe('');
	});

	it('refreshes accepted Save while preserving newer unsaved edits', async () => {
		let finish!: (value: RemoteIndexerConnection) => void;
		vi.spyOn(tauriClient, 'updateRemoteSourceIndexerConnection').mockReturnValue(
			new Promise((resolve) => {
				finish = resolve;
			}),
		);
		vi.spyOn(tauriClient, 'getRemoteSourceAccountState').mockResolvedValue({
			providerId: 'indexer',
			status: 'connected',
		});
		runtime = createTestAppRuntime();
		const owner = runtime.remoteSource;
		vi.spyOn(tauriClient, 'listRemoteSourceProviders').mockResolvedValue([]);
		vi.mocked(tauriClient.getRemoteSourceAccountState).mockResolvedValueOnce({
			providerId: 'indexer',
			status: 'needsAuth',
		});
		await owner.open({ lane: 'indexer' });
		expect(owner.view().accountState?.status).toBe('needsAuth');
		owner.patchIndexerConnectionSettings({
			baseUrlDraft: configured.baseUrl!,
			apiKeyDraft: 'first-key',
		});
		const saving = owner.saveIndexerConnectionSettings();
		owner.patchIndexerConnectionSettings({ apiKeyDraft: 'next-key' });
		finish(configured);
		await saving;
		expect(owner.view().accountState?.status).toBe('connected');
		expect(owner.indexerConnection().apiKeyDraft).toBe('next-key');
		expect(owner.indexerConnection().saveState).toBe('idle');
	});
});
