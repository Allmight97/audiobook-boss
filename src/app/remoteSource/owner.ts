import { createSignal, type Accessor } from 'solid-js';
import type { AudioFile, SupplementalProcessingAsset } from '../../types/audio';
import { tauriClient } from '../../lib/tauri/client';
import type { InputOwner } from '../inputSession';
import {
	createRemoteSourceCoverPreviews,
	type RemoteSourceCoverPreviewState,
} from './coverPreview';
import { makeProductionRemoteSourceServices } from './services';
import { createRemoteSourceSessionAssets, type CompanionAssetSummary } from './sessionAssets';
import { createRemoteSourceStateStore } from './state';
import { createInitialRemoteSourceState, type RemoteSourceView } from './types';
import {
	createRemoteSourceWorkflow,
	type RemoteSourceWorkflowAction,
	type RemoteSourceWorkflowServices,
} from './workflow';

type InputId = string | undefined;

export type RemoteSourceOwner = {
	readonly view: Accessor<RemoteSourceView>;
	open(): void;
	close(): void;
	patch(patch: Partial<RemoteSourceView>): void;
	runAction(action: RemoteSourceWorkflowAction): Promise<void>;
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
	const [viewRev, bumpView] = createSignal(0, { ownedWrite: true });
	const [assetRev, bumpAssets] = createSignal(0, { ownedWrite: true });
	const [previewRev, bumpPreviews] = createSignal(0, { ownedWrite: true });
	const state = createRemoteSourceStateStore(() => {
		snapshot = state.snapshot();
		bumpView((revision) => revision + 1);
	});
	snapshot = state.snapshot();

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
		open() {
			state.patch({ isOpen: true });
		},
		close() {
			state.patch({ isOpen: false, didHydrateOpenDialog: false });
		},
		patch(patch) {
			state.patch(patch);
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
		reconcileWithInput(files) {
			return assets.reconcileInput(files);
		},
		reset() {
			workflow.invalidate();
			previews.clear();
			assets.reset();
			state.reset();
		},
	};
}
