import { createSignal, onCleanup, type Accessor } from 'solid-js';
import type { AcquisitionLane } from '../../types/appSettings';
import { tauriClient } from '../../lib/tauri/client';
import type { RemoteUiIntent } from '../../types/session';
import type { RemoteRelease } from '../../types/remoteSource';
import {
	createIndexerConnectionSettings,
	type IndexerConnectionSettingsView,
} from './indexerConnection';
import type { EngineLink } from '../engineLink';
import { createRemoteSourceStateStore } from './state';
import type { RemoteSourceState, RemoteSourceView } from './types';

/** Semantic actions sent to the engine; browser opening belongs to this frontend. */
export type RemoteSourceAction =
	| { readonly type: 'startAuth' | 'completeAuth' | 'logout' | 'loadLibrary' }
	| { readonly type: 'searchReleases' }
	| { readonly type: 'grabSelectedReleases' }
	| { readonly type: 'grabRelease'; readonly release: Pick<RemoteRelease, 'guid' | 'indexerId'> }
	| { readonly type: 'acquireSelected' }
	| { readonly type: 'cancelActiveAcquisition' };

function accountIntent(
	action: 'startAuth' | 'completeAuth' | 'logout' | 'loadLibrary',
	handoffPath: string,
): RemoteUiIntent {
	switch (action) {
		case 'startAuth':
			return { kind: 'startAuth' };
		case 'completeAuth':
			return { kind: 'completeAuth', responseUrlHandoffPath: handoffPath.trim() || null };
		case 'logout':
			return { kind: 'disconnect', provider: 'audible' };
		case 'loadLibrary':
			return { kind: 'refreshLibrary' };
	}
}

function actionIntent(action: RemoteSourceAction, view: RemoteSourceState): RemoteUiIntent | null {
	switch (action.type) {
		case 'searchReleases':
			return {
				kind: 'searchReleases',
				author: view.indexerAuthorQuery,
				title: view.indexerTitleQuery,
			};
		case 'grabSelectedReleases':
			return { kind: 'grabSelected' };
		case 'grabRelease':
			return {
				kind: 'grabRelease',
				indexerId: action.release.indexerId,
				guid: action.release.guid,
			};
		case 'acquireSelected':
			return { kind: 'acquireSelected' };
		case 'cancelActiveAcquisition':
			return view.activeJob ? { kind: 'cancelAcquisition', jobId: view.activeJob.jobId } : null;
		default:
			return accountIntent(action.type, view.handoffPath);
	}
}

export type RemoteSourceOwner = {
	readonly view: Accessor<RemoteSourceView>;
	readonly indexerConnection: Accessor<IndexerConnectionSettingsView>;
	open(options?: { readonly lane?: AcquisitionLane }): Promise<void>;
	selectLane(lane: AcquisitionLane): Promise<void>;
	close(): void;
	editSearch(
		patch: Partial<
			Pick<
				RemoteSourceView,
				| 'handoffPath'
				| 'titleFilter'
				| 'showSupplementalPdfOnly'
				| 'hideUnavailableTitles'
				| 'indexerAuthorQuery'
				| 'indexerTitleQuery'
				| 'releaseFilter'
				| 'releaseSort'
			>
		>,
	): void;
	toggleTitle(titleId: string): void;
	clearTitleSelection(): void;
	toggleSupplementalPdf(titleId: string): void;
	selectRelease(
		release: Pick<RemoteRelease, 'guid' | 'indexerId'>,
		options?: { multi: boolean },
	): void;
	runAction(action: RemoteSourceAction): Promise<void>;
	loadIndexerConnectionSettings(): Promise<void>;
	patchIndexerConnectionSettings(
		patch: Partial<
			Pick<IndexerConnectionSettingsView, 'baseUrlDraft' | 'apiKeyDraft' | 'categoryIdsDraft'>
		>,
	): void;
	saveIndexerConnectionSettings(): Promise<void>;
	testIndexerConnection(): Promise<void>;
	reset(): void;
};

export type RemoteSourceOwnerDeps = {
	readonly link: EngineLink;
	readonly openAuthorizationUrl?: (url: string) => Promise<void>;
};

export function createRemoteSourceOwner(deps: RemoteSourceOwnerDeps): RemoteSourceOwner {
	const [viewRev, bumpView] = createSignal(0, { ownedWrite: true });
	const state = createRemoteSourceStateStore(
		() => bumpView((revision) => revision + 1),
		() => deps.link.remote(),
		() => deps.link.remoteLibrary(),
	);
	const indexerConnection = createIndexerConnectionSettings(deps.link);

	const openAuthorizationUrl = deps.openAuthorizationUrl ?? tauriClient.openUrl;
	let generation = 0;
	let disposed = false;
	onCleanup(() => {
		disposed = true;
		generation += 1;
	});

	return {
		view: () => {
			viewRev();
			return state.snapshot();
		},
		indexerConnection: indexerConnection.view,
		async open(options) {
			if (disposed) return;
			const started = generation;
			state.patch({ isOpen: true });
			const lane = options?.lane ?? deps.link.remote().lane;
			state.clearError(lane);
			try {
				await deps.link.send({ kind: 'remote', intent: { kind: 'selectLane', lane } });
			} catch (error) {
				if (!disposed && started === generation)
					state.setAcquisitionError(error, 'Could not open Remote Source.', lane);
			}
		},
		async selectLane(lane) {
			if (disposed) return;
			const started = generation;
			state.clearError(lane);
			try {
				await deps.link.send({ kind: 'remote', intent: { kind: 'selectLane', lane } });
			} catch (error) {
				if (!disposed && started === generation)
					state.setAcquisitionError(error, 'Could not change Remote Source.', lane);
			}
		},
		close() {
			state.patch({ isOpen: false });
		},
		editSearch(patch) {
			state.patch(patch);
		},
		toggleTitle(titleId) {
			deps.link.post({ kind: 'remote', intent: { kind: 'toggleTitle', titleId } });
		},
		clearTitleSelection() {
			deps.link.post({ kind: 'remote', intent: { kind: 'clearTitles' } });
		},
		toggleSupplementalPdf(titleId) {
			deps.link.post({ kind: 'remote', intent: { kind: 'togglePdf', titleId } });
		},
		selectRelease(identity, options) {
			deps.link.post({
				kind: 'remote',
				intent: {
					kind: 'selectRelease',
					indexerId: identity.indexerId,
					guid: identity.guid,
					multi: options?.multi ?? false,
				},
			});
		},
		async runAction(action) {
			const providerId = state.current().providerId;
			const started = generation;
			if (disposed) return;
			state.clearError(providerId);
			try {
				const intent = actionIntent(action, state.current());
				if (!intent) return;
				const outcome = await deps.link.send({ kind: 'remote', intent });
				if (
					action.type === 'startAuth' &&
					!disposed &&
					started === generation &&
					outcome.kind === 'remoteAuthStarted'
				) {
					await openAuthorizationUrl(outcome.authorization.authorizationUrl);
				}
			} catch (error) {
				if (!disposed && started === generation)
					state.setAcquisitionError(error, 'Remote source request failed.', providerId);
			}
		},
		loadIndexerConnectionSettings() {
			return indexerConnection.load();
		},
		patchIndexerConnectionSettings(patch) {
			indexerConnection.patch(patch);
		},
		async saveIndexerConnectionSettings() {
			if (indexerConnection.isSaving()) return;

			await indexerConnection.save();
		},
		testIndexerConnection() {
			return indexerConnection.testConnection();
		},
		reset() {
			generation += 1;
			indexerConnection.reset();
			state.reset();
		},
	};
}
