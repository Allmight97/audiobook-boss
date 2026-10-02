import { createSignal, type Accessor } from 'solid-js';
import type { AcquisitionLane } from '../../types/appSettings';
import { tauriClient } from '../../lib/tauri/client';
import type { RemoteRelease } from '../../types/remoteSource';
import {
	releaseKey,
	toggledRemoteTitleSelection,
	toggledSupplementalPdfPreference,
} from './selection';
import {
	createCoverArtPreviewScheduler,
	type CoverArtPreviewState,
} from '../../lib/media/coverArtPreviewScheduler';
import {
	createIndexerConnectionSettings,
	type IndexerConnectionSettingsView,
} from './indexerConnection';
import {
	makeProductionIndexerConnectionServices,
	makeProductionRemoteSourceServices,
} from './services';
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
	readonly services?: RemoteSourceWorkflowServices;
	readonly loadCoverArtFromUrl?: (url: string) => Promise<number[]>;
};

export function createRemoteSourceOwner(deps: RemoteSourceOwnerDeps): RemoteSourceOwner {
	let snapshot: RemoteSourceView;
	const [viewRev, bumpView] = createSignal(0, { ownedWrite: true });
	const [previewRev, bumpPreviews] = createSignal(0, { ownedWrite: true });
	const state = createRemoteSourceStateStore(() => {
		snapshot = state.snapshot();
		bumpView((revision) => revision + 1);
	});
	snapshot = state.snapshot();
	const indexerConnection = createIndexerConnectionSettings({
		services: makeProductionIndexerConnectionServices,
	});

	const services = deps.services ?? makeProductionRemoteSourceServices();
	const previews = createCoverArtPreviewScheduler({
		load: deps.loadCoverArtFromUrl ?? tauriClient.loadCoverArtFromUrl,
		onChange: () => bumpPreviews((revision) => revision + 1),
		failureLogMessage: 'Failed to load remote source cover preview:',
	});
	const workflow = createRemoteSourceWorkflow({ services, state });

	return {
		view: () => {
			viewRev();
			return snapshot;
		},
		indexerConnection: indexerConnection.view,
		async open(options) {
			state.patch({ isOpen: true });
			await workflow.run({ type: 'enterLane', lane: options?.lane ?? 'audible' });
		},
		selectLane(lane) {
			return workflow.run({ type: 'enterLane', lane });
		},
		close() {
			state.patch({ isOpen: false });
		},
		editSearch(patch) {
			state.patch(patch);
		},
		toggleTitle(titleId) {
			const current = state.current();
			const title = current.titles.find((item) => item.titleId === titleId);
			if (title)
				state.patch({
					selectedTitleIds: toggledRemoteTitleSelection(current.selectedTitleIds, title),
				});
		},
		clearTitleSelection() {
			state.patch({ selectedTitleIds: new Set() });
		},
		toggleSupplementalPdf(titleId) {
			const current = state.current();
			if (
				current.titles.some((title) => title.titleId === titleId && title.supplementalPdfAvailable)
			) {
				state.patch({
					includePdfByTitleId: toggledSupplementalPdfPreference(
						current.includePdfByTitleId,
						titleId,
					),
				});
			}
		},
		selectRelease(identity, options) {
			const current = state.current();
			const key = releaseKey(identity);
			if (!current.releases.some((release) => releaseKey(release) === key)) return;
			const selected = options?.multi ? new Set(current.selectedReleaseKeys) : new Set<string>();
			if (options?.multi && selected.has(key)) selected.delete(key);
			else selected.add(key);
			state.patch({ selectedReleaseKeys: selected });
		},
		async runAction(action) {
			if (
				indexerConnection.isSaving() &&
				(action.type === 'searchReleases' ||
					action.type === 'grabRelease' ||
					action.type === 'grabSelectedReleases')
			) {
				state.patch(
					{
						statusMessage:
							'Wait for the Indexer connection save to finish before searching or grabbing.',
					},
					'indexer',
				);
				return;
			}
			try {
				await workflow.run(action);
			} catch (error) {
				console.error('Remote source workflow failed:', error);
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
			if (state.current().isGrabbing) {
				await indexerConnection.save(
					'Wait for the current Grab batch to finish before saving the Indexer connection.',
				);
				return;
			}
			const saved = await indexerConnection.save();
			if (saved) {
				workflow.clearIndexerResults();
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
			workflow.invalidate();
			previews.clear();
			indexerConnection.reset();
			state.reset();
		},
	};
}
