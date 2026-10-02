import { createSignal, onCleanup, type Accessor } from 'solid-js';
import type { AcquisitionLane } from '../../types/appSettings';
import { tauriClient } from '../../lib/tauri/client';
import type { RemoteRelease } from '../../types/remoteSource';
import {
	createCoverArtPreviewScheduler,
	type CoverArtPreviewState,
} from '../../lib/media/coverArtPreviewScheduler';
import {
	createIndexerConnectionSettings,
	type IndexerConnectionSettingsView,
} from './indexerConnection';
import { makeProductionRemoteSourceServices } from './services';
import type { EngineLink } from '../engineLink';
import { createRemoteSourceStateStore } from './state';
import type { RemoteSourceView } from './types';
import {
	createRemoteSourceWorkflow,
	type RemoteSourceWorkflowAction,
	type RemoteSourceWorkflowServices,
} from './workflow';

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
	runAction(
		action: Exclude<RemoteSourceWorkflowAction, { type: 'enterLane' | 'refreshAccount' }>,
	): Promise<void>;
	coverPreview(coverUrl: string | null | undefined): CoverArtPreviewState;
	scheduleCoverPreviews(coverUrls: ReadonlyArray<string | null | undefined>): void;
	cancelCoverPreviews(): void;
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
	readonly services?: RemoteSourceWorkflowServices;
	readonly loadCoverArtFromUrl?: (url: string) => Promise<number[]>;
};

export function createRemoteSourceOwner(deps: RemoteSourceOwnerDeps): RemoteSourceOwner {
	const [viewRev, bumpView] = createSignal(0, { ownedWrite: true });
	const [previewRev, bumpPreviews] = createSignal(0, { ownedWrite: true });
	const state = createRemoteSourceStateStore(
		() => bumpView((revision) => revision + 1),
		() => deps.link.remote(),
	);
	const indexerConnection = createIndexerConnectionSettings(deps.link);

	const services = deps.services ?? makeProductionRemoteSourceServices();
	const previews = createCoverArtPreviewScheduler({
		load: deps.loadCoverArtFromUrl ?? tauriClient.loadCoverArtFromUrl,
		onChange: () => bumpPreviews((revision) => revision + 1),
		failureLogMessage: 'Failed to load remote source cover preview:',
	});
	let generation = 0;
	let disposed = false;
	onCleanup(() => {
		disposed = true;
		generation += 1;
	});
	const workflow = createRemoteSourceWorkflow({ services, state });

	return {
		view: () => {
			viewRev();
			return state.snapshot();
		},
		indexerConnection: indexerConnection.view,
		async open(options) {
			const started = generation;
			state.patch({ isOpen: true });
			const lane = options?.lane ?? deps.link.remote().lane;
			try {
				await deps.link.send({ kind: 'remote', intent: { kind: 'selectLane', lane } });
				if (disposed || started !== generation) return;
				await workflow.run({ type: 'enterLane', lane });
			} catch (error) {
				if (!disposed && started === generation)
					state.setAcquisitionError(error, 'Could not open Remote Source.', lane);
			}
		},
		async selectLane(lane) {
			const started = generation;
			try {
				await deps.link.send({ kind: 'remote', intent: { kind: 'selectLane', lane } });
				if (disposed || started !== generation) return;
				await workflow.run({ type: 'enterLane', lane });
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
			state.clearError(providerId);
			try {
				if (action.type === 'searchReleases') {
					const view = state.current();
					await deps.link.send({
						kind: 'remote',
						intent: {
							kind: 'searchReleases',
							author: view.indexerAuthorQuery,
							title: view.indexerTitleQuery,
						},
					});
				} else if (action.type === 'grabSelectedReleases') {
					await deps.link.send({ kind: 'remote', intent: { kind: 'grabSelected' } });
				} else if (action.type === 'grabRelease') {
					await deps.link.send({
						kind: 'remote',
						intent: {
							kind: 'grabRelease',
							indexerId: action.release.indexerId,
							guid: action.release.guid,
						},
					});
				} else if (action.type === 'acquireSelected') {
					await deps.link.send({ kind: 'remote', intent: { kind: 'acquireSelected' } });
				} else if (action.type === 'cancelActiveAcquisition') {
					const job = deps.link.remote().acquisition;
					if (job)
						await deps.link.send({
							kind: 'remote',
							intent: { kind: 'cancelAcquisition', jobId: job.jobId },
						});
				} else await workflow.run(action);
			} catch (error) {
				state.setAcquisitionError(error, 'Remote source request failed.', providerId);
			}
		},
		coverPreview(coverUrl) {
			previewRev();
			return previews.getState(coverUrl);
		},
		scheduleCoverPreviews(coverUrls) {
			previews.schedule(coverUrls);
		},
		cancelCoverPreviews() {
			previews.cancel();
		},
		loadIndexerConnectionSettings() {
			return indexerConnection.load();
		},
		patchIndexerConnectionSettings(patch) {
			indexerConnection.patch(patch);
		},
		async saveIndexerConnectionSettings() {
			if (indexerConnection.isSaving()) return;

			const saved = await indexerConnection.save();
			if (saved) {
				state.patch(
					{ statusMessage: 'Indexer connection saved. Search again before grabbing.' },
					'indexer',
				);
			}
			if (saved && state.current().isOpen && state.current().providerId === 'indexer') {
				await workflow.run({ type: 'refreshAccount' });
			}
		},
		testIndexerConnection() {
			return indexerConnection.testConnection();
		},
		reset() {
			generation += 1;
			workflow.invalidate();
			previews.clear();
			indexerConnection.reset();
			state.reset();
		},
	};
}
