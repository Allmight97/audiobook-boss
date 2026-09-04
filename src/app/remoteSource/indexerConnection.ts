import { createSignal, type Accessor } from 'solid-js';
import { toUserMessage } from '../../lib/tauri/appError';
import type {
	RemoteIndexerConnection,
	RemoteIndexerConnectionUpdate,
} from '../../types/remoteSource';

export type IndexerConnectionSaveState = 'idle' | 'saving' | 'saved' | 'error';
export type IndexerConnectionTestState = 'idle' | 'testing' | 'success' | 'error';

export type IndexerConnectionSettingsView = {
	baseUrlDraft: string;
	categoryIdDraft: number;
	apiKeyDraft: string;
	apiKeyConfigured: boolean;
	saveState: IndexerConnectionSaveState;
	saveError: string;
	testState: IndexerConnectionTestState;
	testMessage: string;
};

export type IndexerConnectionServices = {
	getIndexerConnection: () => Promise<RemoteIndexerConnection>;
	updateIndexerConnection: (
		update: RemoteIndexerConnectionUpdate,
	) => Promise<RemoteIndexerConnection>;
	testIndexerConnection: () => Promise<{ ok: boolean; message: string }>;
};

function createInitialView(): IndexerConnectionSettingsView {
	return {
		baseUrlDraft: '',
		categoryIdDraft: 3030,
		apiKeyDraft: '',
		apiKeyConfigured: false,
		saveState: 'idle',
		saveError: '',
		testState: 'idle',
		testMessage: '',
	};
}

function applyConnectionToDraft(
	draft: IndexerConnectionSettingsView,
	connection: RemoteIndexerConnection,
): void {
	draft.baseUrlDraft = connection.baseUrl ?? '';
	draft.categoryIdDraft = connection.categoryId;
	draft.apiKeyConfigured = connection.apiKeyConfigured;
	draft.apiKeyDraft = '';
}

export function createIndexerConnectionSettings(deps: {
	readonly services: () => IndexerConnectionServices;
}): {
	readonly view: Accessor<IndexerConnectionSettingsView>;
	load(): Promise<void>;
	patch(patch: Partial<IndexerConnectionSettingsView>): void;
	save(): Promise<void>;
	testConnection(): Promise<void>;
	reset(): void;
} {
	const [view, setView] = createSignal(createInitialView());

	function update(mutator: (draft: IndexerConnectionSettingsView) => void): void {
		const next = { ...view() };
		mutator(next);
		setView(next);
	}

	return {
		view,
		async load() {
			try {
				const connection = await deps.services().getIndexerConnection();
				update((draft) => {
					applyConnectionToDraft(draft, connection);
					draft.saveState = 'idle';
					draft.saveError = '';
					draft.testState = 'idle';
					draft.testMessage = '';
				});
			} catch (cause) {
				update((draft) => {
					draft.saveState = 'error';
					draft.saveError = toUserMessage(cause, {
						fallback: 'Failed to load Indexer connection settings.',
					});
				});
			}
		},
		patch(patch) {
			update((draft) => {
				Object.assign(draft, patch);
			});
		},
		async save() {
			const current = view();
			update((draft) => {
				draft.saveState = 'saving';
				draft.saveError = '';
			});
			try {
				const trimmedUrl = current.baseUrlDraft.trim();
				const connectionUpdate: RemoteIndexerConnectionUpdate = {
					baseUrl: trimmedUrl.length > 0 ? trimmedUrl : '',
					categoryId: current.categoryIdDraft,
				};
				const trimmedKey = current.apiKeyDraft.trim();
				if (trimmedKey.length > 0) {
					connectionUpdate.apiKey = trimmedKey;
				}
				const connection = await deps.services().updateIndexerConnection(connectionUpdate);
				update((draft) => {
					applyConnectionToDraft(draft, connection);
					draft.saveState = 'saved';
				});
			} catch (cause) {
				update((draft) => {
					draft.saveState = 'error';
					draft.saveError = toUserMessage(cause, {
						fallback: 'Failed to save Indexer connection settings.',
					});
				});
			}
		},
		async testConnection() {
			update((draft) => {
				draft.testState = 'testing';
				draft.testMessage = '';
			});
			try {
				const result = await deps.services().testIndexerConnection();
				update((draft) => {
					draft.testState = result.ok ? 'success' : 'error';
					draft.testMessage = result.message;
				});
			} catch (cause) {
				update((draft) => {
					draft.testState = 'error';
					draft.testMessage = toUserMessage(cause, {
						fallback: 'Indexer connection test failed.',
					});
				});
			}
		},
		reset() {
			setView(createInitialView());
		},
	};
}
