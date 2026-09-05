import { createSignal, type Accessor } from 'solid-js';
import type { AcquisitionLane } from '../../types/appSettings';
import type { AudioFile, SupplementalProcessingAsset } from '../../types/audio';
import { tauriClient } from '../../lib/tauri/client';
import type { RemoteRelease } from '../../types/remoteSource';
import { toggledRemoteTitleSelection, toggledSupplementalPdfPreference } from './selection';
import type { InputOwner } from '../inputSession';
import {
	createRemoteSourceCoverPreviews,
	type RemoteSourceCoverPreviewState,
} from './coverPreview';
import {
	createIndexerConnectionSettings,
	type IndexerConnectionSettingsView,
} from './indexerConnection';
import {
	makeProductionIndexerConnectionServices,
	makeProductionRemoteSourceServices,
} from './services';
import { createRemoteSourceSessionAssets, type CompanionAssetSummary } from './sessionAssets';
import { createRemoteSourceStateStore } from './state';
import {
	createInitialRemoteSourceState,
	laneSelectionResetPatch,
	providerIdFromLane,
	type RemoteSourceView,
} from './types';
import {
	createRemoteSourceWorkflow,
	type RemoteSourceWorkflowAction,
	type RemoteSourceWorkflowServices,
} from './workflow';

type InputId = string | undefined;

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
	selectRelease(release: Pick<RemoteRelease, 'guid' | 'indexerId'>): void;
	runAction(
		action: Exclude<
			RemoteSourceWorkflowAction,
			{ type: 'hydrateOpenDialog' | 'refreshAccount' | 'selectLane' }
		>,
	): Promise<void>;
	coverPreview(coverUrl: string | null | undefined): RemoteSourceCoverPreviewState;
	scheduleCoverPreviews(coverUrls: ReadonlyArray<string | null | undefined>): void;
	cancelCoverPreviews(): void;
	companionSummary(inputIds: readonly InputId[]): CompanionAssetSummary;
	hasCompanions(inputId: InputId): boolean;
	processingAssets(
		inputIds: readonly InputId[],
	): Record<string, SupplementalProcessingAsset[]> | undefined;
	withSubmissionRetention<T>(inputIds: readonly InputId[], submit: () => Promise<T>): Promise<T>;
	settleTerminalWork(input: {
		readonly inputIds: readonly string[];
		readonly completedInputIds: readonly string[];
	}): Promise<void>;
	loadIndexerConnectionSettings(): Promise<void>;
	patchIndexerConnectionSettings(
		patch: Partial<
			Pick<IndexerConnectionSettingsView, 'baseUrlDraft' | 'apiKeyDraft' | 'categoryIdsDraft'>
		>,
	): void;
	saveIndexerConnectionSettings(): Promise<void>;
	testIndexerConnection(): Promise<void>;
	reconcileWithInput(files: ReadonlyArray<AudioFile>): Promise<void>;
	reset(): void;
};

export type RemoteSourceOwnerDeps = {
	readonly input: InputOwner;
	readonly services?: RemoteSourceWorkflowServices;
	readonly loadCoverArtFromUrl?: (url: string) => Promise<number[]>;
};

export function createRemoteSourceOwner(deps: RemoteSourceOwnerDeps): RemoteSourceOwner {
	let snapshot = createInitialRemoteSourceState();
	const [viewRev, bumpView] = createSignal(0);
	const [assetRev, bumpAssets] = createSignal(0);
	const [previewRev, bumpPreviews] = createSignal(0);
	const state = createRemoteSourceStateStore(() => {
		snapshot = state.snapshot();
		bumpView((revision) => revision + 1);
	});
	snapshot = state.snapshot();
	const indexerConnection = createIndexerConnectionSettings({
		services: makeProductionIndexerConnectionServices,
	});

	const services =
		deps.services ??
		makeProductionRemoteSourceServices({
			inputView: () => deps.input.view(),
			importPaths: (paths) => deps.input.importIntent({ type: 'importPaths', paths }),
		});
	const assets = createRemoteSourceSessionAssets({
		purgeSession: services.purgeSession,
		onChange: () => bumpAssets((revision) => revision + 1),
	});
	const previews = createRemoteSourceCoverPreviews({
		loadCoverArtFromUrl: deps.loadCoverArtFromUrl ?? tauriClient.loadCoverArtFromUrl,
		onChange: () => bumpPreviews((revision) => revision + 1),
	});
	const workflow = createRemoteSourceWorkflow({
		services,
		state,
		registerSupplementalAssets: assets.register,
	});

	return {
		view: () => {
			viewRev();
			return snapshot;
		},
		indexerConnection: indexerConnection.view,
		async open(options) {
			const lane = options?.lane ?? 'audible';
			state.patch({
				isOpen: true,
				providerId: providerIdFromLane(lane),
				...(state.current().providerId !== providerIdFromLane(lane)
					? { ...laneSelectionResetPatch(), accountState: null }
					: {}),
			});
			await workflow.run({ type: 'hydrateOpenDialog' });
		},
		selectLane(lane) {
			return workflow.run({ type: 'selectLane', lane });
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
		selectRelease(identity) {
			const release = state
				.current()
				.releases.find(
					(item) => item.guid === identity.guid && item.indexerId === identity.indexerId,
				);
			if (release)
				state.patch({ selectedRelease: { guid: release.guid, indexerId: release.indexerId } });
		},
		async runAction(action) {
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
		companionSummary(inputIds) {
			assetRev();
			return assets.companionSummary(inputIds);
		},
		hasCompanions(inputId) {
			assetRev();
			return assets.hasCompanions(inputId);
		},
		processingAssets(inputIds) {
			return assets.processingAssets(inputIds);
		},
		withSubmissionRetention(inputIds, submit) {
			return assets.withSubmissionRetention(inputIds, submit);
		},
		settleTerminalWork(input) {
			return assets.settleTerminalWork(input);
		},
		loadIndexerConnectionSettings() {
			return indexerConnection.load();
		},
		patchIndexerConnectionSettings(patch) {
			indexerConnection.patch(patch);
		},
		async saveIndexerConnectionSettings() {
			const saved = await indexerConnection.save();
			if (saved && state.current().isOpen && state.current().providerId === 'indexer') {
				await workflow.run({ type: 'refreshAccount' });
			}
		},
		testIndexerConnection() {
			return indexerConnection.testConnection();
		},
		reconcileWithInput(files) {
			return assets.reconcileInput(files);
		},
		reset() {
			workflow.invalidate();
			previews.clear();
			assets.reset();
			indexerConnection.reset();
			state.reset();
		},
	};
}
