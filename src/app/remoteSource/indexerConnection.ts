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
	categoryIdsDraft: number[];
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
	testIndexerConnection: (
		update: RemoteIndexerConnectionUpdate,
	) => Promise<{ ok: boolean; message: string }>;
};

function createInitialView(): IndexerConnectionSettingsView {
	return {
		baseUrlDraft: '',
		categoryIdsDraft: [3000, 3030],
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
	draft.categoryIdsDraft = [...connection.categoryIds];
	draft.apiKeyConfigured = connection.apiKeyConfigured;
	draft.apiKeyDraft = '';
}

export function createIndexerConnectionSettings(deps: {
	readonly services: () => IndexerConnectionServices;
}): {
	readonly view: Accessor<IndexerConnectionSettingsView>;
	load(): Promise<void>;
	patch(patch: Partial<IndexerConnectionSettingsView>): void;
	save(): Promise<boolean>;
	testConnection(): Promise<void>;
	reset(): void;
} {
	const [view, setView] = createSignal(createInitialView());
	let draftRevision = 0;
	let lifetimeRevision = 0;

	function update(mutator: (draft: IndexerConnectionSettingsView) => void): void {
		const next = { ...view() };
		mutator(next);
		setView(next);
	}

	function draftUpdate(): RemoteIndexerConnectionUpdate {
		const current = view();
		const apiKey = current.apiKeyDraft.trim();
		return {
			baseUrl: current.baseUrlDraft.trim(),
			categoryIds: [...current.categoryIdsDraft],
			...(apiKey ? { apiKey } : {}),
		};
	}

	return {
		view,
		async load() {
			const revision = ++draftRevision;
			try {
				const connection = await deps.services().getIndexerConnection();
				if (revision !== draftRevision) return;
				update((draft) => {
					applyConnectionToDraft(draft, connection);
					draft.saveState = 'idle';
					draft.saveError = '';
					draft.testState = 'idle';
					draft.testMessage = '';
				});
			} catch (cause) {
				if (revision !== draftRevision) return;
				update((draft) => {
					draft.saveState = 'error';
					draft.saveError = toUserMessage(cause, {
						fallback: 'Failed to load Indexer connection settings.',
					});
				});
			}
		},
		patch(patch) {
			draftRevision += 1;
			update((draft) => {
				Object.assign(draft, patch);
				draft.saveState = 'idle';
				draft.saveError = '';
				draft.testState = 'idle';
				draft.testMessage = '';
			});
		},
		async save() {
			const lifetime = lifetimeRevision;
			const revision = ++draftRevision;
			const connectionUpdate = draftUpdate();
			update((draft) => {
				draft.saveState = 'saving';
				draft.saveError = '';
				draft.testState = 'idle';
				draft.testMessage = '';
			});
			try {
				const connection = await deps.services().updateIndexerConnection(connectionUpdate);
				if (lifetime !== lifetimeRevision) return false;
				if (revision !== draftRevision) {
					update((draft) => {
						draft.apiKeyConfigured = connection.apiKeyConfigured;
					});
					return true;
				}
				update((draft) => {
					applyConnectionToDraft(draft, connection);
					draft.saveState = 'saved';
				});
				return true;
			} catch (cause) {
				if (lifetime !== lifetimeRevision) return false;
				update((draft) => {
					draft.saveState = 'error';
					draft.saveError = toUserMessage(cause, {
						fallback: 'Failed to save Indexer connection settings.',
					});
				});
				return false;
			}
		},
		async testConnection() {
			const revision = draftRevision;
			const connectionUpdate = draftUpdate();
			update((draft) => {
				draft.testState = 'testing';
				draft.testMessage = '';
			});
			try {
				const result = await deps.services().testIndexerConnection(connectionUpdate);
				if (revision !== draftRevision) return;
				update((draft) => {
					draft.testState = result.ok ? 'success' : 'error';
					draft.testMessage = result.message;
				});
			} catch (cause) {
				if (revision !== draftRevision) return;
				update((draft) => {
					draft.testState = 'error';
					draft.testMessage = toUserMessage(cause, {
						fallback: 'Indexer connection test failed.',
					});
				});
			}
		},
		reset() {
			lifetimeRevision += 1;
			draftRevision += 1;
			setView(createInitialView());
		},
	};
}
