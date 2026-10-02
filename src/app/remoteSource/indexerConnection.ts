import { createEffect, createSignal, type Accessor } from 'solid-js';
import { toUserMessage } from '../../lib/tauri/appError';
import type { EngineLink } from '../engineLink';

export type IndexerConnectionSettingsView = {
	baseUrlDraft: string;
	categoryIdsDraft: number[];
	apiKeyDraft: string;
	apiKeyConfigured: boolean;
	saveState: 'idle' | 'saving' | 'saved' | 'error';
	saveError: string;
	testState: 'idle' | 'testing' | 'success' | 'error';
	testMessage: string;
};

/** Only unconfirmed typing and the entered key's visual echo live here. */
export function createIndexerConnectionSettings(link: EngineLink): {
	readonly view: Accessor<IndexerConnectionSettingsView>;
	load(): Promise<void>;
	patch(
		patch: Partial<
			Pick<IndexerConnectionSettingsView, 'baseUrlDraft' | 'categoryIdsDraft' | 'apiKeyDraft'>
		>,
	): void;
	isSaving(): boolean;
	save(): Promise<boolean>;
	testConnection(): Promise<void>;
	reset(): void;
} {
	const [revision, bump] = createSignal(0, { ownedWrite: true });
	let typed: { baseUrlDraft?: string; categoryIdsDraft?: number[] } | null = null;
	let keyEcho = '';
	let keyAccepted = false;
	let localError = '';
	const changed = () => bump((value) => value + 1);
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
	async function send(kind: 'loadConnection' | 'saveConnection' | 'testConnection') {
		localError = '';
		changed();
		try {
			return await link.send({ kind: 'remote', intent: { kind } });
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
			const saveError = draft.save.kind === 'failed' ? toUserMessage(draft.save.error) : localError;
			return {
				baseUrlDraft: typed?.baseUrlDraft ?? draft.baseUrl,
				categoryIdsDraft: typed?.categoryIdsDraft ?? draft.categoryIds,
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
			await send('loadConnection');
		},
		patch(patch) {
			const echo = {
				...typed,
				...(patch.baseUrlDraft === undefined ? {} : { baseUrlDraft: patch.baseUrlDraft }),
				...(patch.categoryIdsDraft === undefined
					? {}
					: { categoryIdsDraft: patch.categoryIdsDraft }),
			};
			typed = echo;
			if (patch.apiKeyDraft !== undefined) keyEcho = patch.apiKeyDraft;
			localError = '';
			changed();
			void link
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
				})
				.finally(() => {
					if (
						typed === echo &&
						(echo.baseUrlDraft === undefined ||
							echo.baseUrlDraft.trim() === link.remote().connection.baseUrl) &&
						(echo.categoryIdsDraft === undefined ||
							echo.categoryIdsDraft.join(',') === link.remote().connection.categoryIds.join(','))
					) {
						typed = null;
						changed();
					}
				});
		},
		isSaving: () => link.remote().connection.save.kind === 'running',
		async save() {
			return (await send('saveConnection'))?.kind === 'remoteSaved';
		},
		async testConnection() {
			await send('testConnection');
		},
		reset() {
			typed = null;
			keyEcho = '';
			localError = '';
			changed();
		},
	};
}
