import { createEffect, createSignal, type Accessor } from 'solid-js';
import { toUserMessage } from '../../lib/tauri/appError';
import type { RemoteUiIntent } from '../../lib/generated/tauri';
import { createTypingEcho, type EngineLink } from '../engineLink';

export type IndexerConnectionSettingsView = {
	baseUrlDraft: string;
	categoryIdsDraft: number[];
	categoriesRefused: boolean;
	apiKeyDraft: string;
	apiKeyConfigured: boolean;
	saveState: 'idle' | 'saving' | 'saved' | 'error';
	saveError: string;
	testState: 'idle' | 'testing' | 'success' | 'error';
	testMessage: string;
};

export function createIndexerConnectionSettings(link: EngineLink): {
	readonly view: Accessor<IndexerConnectionSettingsView>;
	load(): Promise<void>;
	patch(
		patch: Partial<
			Pick<IndexerConnectionSettingsView, 'baseUrlDraft' | 'categoryIdsDraft' | 'apiKeyDraft'>
		>,
	): void;
	save(): Promise<void>;
	testConnection(): Promise<void>;
	reset(): void;
} {
	const [revision, bump] = createSignal(0, { ownedWrite: true });
	let keyEcho = '';
	let keyAccepted = false;
	let localError = '';
	const changed = () => bump((value) => value + 1);
	const typed = createTypingEcho<'baseUrl'>({ scopeOf: () => [], changed });
	createEffect(
		() => link.remote().connection.apiKeyEntered,
		(entered) => {
			if (entered) keyAccepted = true;
			else if (keyAccepted) {
				keyAccepted = false;
				keyEcho = '';
				changed();
			}
		},
	);
	async function send(intent: RemoteUiIntent) {
		localError = '';
		changed();
		try {
			return await link.send({ kind: 'remote', intent });
		} catch (error) {
			localError = toUserMessage(error);
			changed();
			return null;
		}
	}
	return {
		view: () => {
			revision();
			const draft = link.remote().connection;
			const saveError =
				draft.save.kind === 'failed'
					? toUserMessage(draft.save.error)
					: draft.draftError
						? toUserMessage(draft.draftError)
						: localError;
			return {
				baseUrlDraft: typed.valueOf('baseUrl') ?? draft.baseUrl,
				categoryIdsDraft: draft.categoryIds,
				categoriesRefused: draft.emptyCategoriesRefused,
				apiKeyDraft: keyEcho,
				apiKeyConfigured: draft.apiKeyConfigured,
				saveState: saveError
					? 'error'
					: draft.save.kind === 'running'
						? 'saving'
						: draft.save.kind === 'succeeded'
							? 'saved'
							: 'idle',
				saveError,
				testState:
					draft.test.kind === 'running'
						? 'testing'
						: draft.test.kind === 'failed' || draft.testResult?.ok === false
							? 'error'
							: draft.test.kind === 'succeeded'
								? 'success'
								: 'idle',
				testMessage:
					draft.test.kind === 'failed'
						? toUserMessage(draft.test.error)
						: (draft.testResult?.message ?? ''),
			};
		},
		async load() {
			await send({ kind: 'loadConnection' });
		},
		patch(patch) {
			if (patch.apiKeyDraft !== undefined) keyEcho = patch.apiKeyDraft;
			localError = '';
			changed();
			const sendEdit = () =>
				link
					.send({
						kind: 'remote',
						intent: {
							kind: 'editConnection',
							baseUrl: patch.baseUrlDraft ?? null,
							categoryIds: patch.categoryIdsDraft ?? null,
							apiKey: patch.apiKeyDraft ?? null,
						},
					})
					.catch((error: unknown) => {
						localError = toUserMessage(error);
						changed();
					});
			if (patch.baseUrlDraft === undefined) void sendEdit();
			else typed.show('baseUrl', patch.baseUrlDraft, sendEdit);
		},
		async save() {
			await send({ kind: 'saveConnection' });
		},
		async testConnection() {
			await send({ kind: 'testConnection' });
		},
		reset() {
			typed.clear();
			keyEcho = '';
			localError = '';
			changed();
		},
	};
}
