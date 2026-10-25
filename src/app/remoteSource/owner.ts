import { createMemo, createSignal, getOwner, onCleanup, type Accessor } from 'solid-js';
import type { AcquisitionLane } from '../../types/appSettings';
import { tauriClient } from '../../lib/tauri/client';
import type { RemoteUiIntent } from '../../types/session';
import type { AcquisitionJob, AcquisitionProgress, RemoteRelease } from '../../types/remoteSource';
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

/** The download bar. Title rows do not read it, so a tick does not rebuild them. */
export type RemoteDownloadProgress = {
	readonly percentage: number;
	readonly progress: AcquisitionProgress;
	readonly terminal: boolean;
	readonly active: boolean;
};

export type RemoteSourceOwner = {
	readonly view: Accessor<RemoteSourceView>;
	readonly progress: Accessor<RemoteDownloadProgress | null>;
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

	let shown: RemoteSourceView | undefined;
	const viewMemo = createMemo(() => {
		viewRev();
		return currentView();
	});
	const progressMemo = createMemo(() =>
		currentProgress(deps.link.remote(), deps.link.remoteProgress()),
	);
	// Solid 2 returns the memo's last committed value outside a render; this read does not replace `shown`.
	const view = () => (getOwner() ? viewMemo() : screenNow());
	const progress = () =>
		getOwner() ? progressMemo() : currentProgress(deps.link.remote(), deps.link.remoteProgress());

	function screenNow(): RemoteSourceView {
		const next = state.snapshot();
		return shown && sameScreen(shown, next) ? shown : next;
	}

	function currentProgress(
		remote: ReturnType<EngineLink['remote']>,
		tick: ReturnType<EngineLink['remoteProgress']>,
	): RemoteDownloadProgress | null {
		const job = remote.acquisition;
		if (!job) return null;
		const reading = tick && tick.jobId === job.jobId ? tick.progress : job.progress;
		return {
			percentage: Math.max(0, Math.min(100, reading.percentage)),
			progress: reading,
			terminal: job.terminal,
			active: remote.acquiring || !job.settled,
		};
	}

	function currentView(): RemoteSourceView {
		const next = state.snapshot();
		if (shown && sameScreen(shown, next)) return shown;
		shown = next;
		return next;
	}

	return {
		view,
		progress,
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

/** A download tick changes the fill, not the library the rows are drawn from. */
function sameScreen(previous: RemoteSourceView, next: RemoteSourceView): boolean {
	const same: Record<keyof RemoteSourceView, boolean> = {
		isOpen: previous.isOpen === next.isOpen,
		isBusy: previous.isBusy === next.isBusy,
		providerId: previous.providerId === next.providerId,
		providers: sameJson(previous.providers, next.providers),
		accountState: sameJson(previous.accountState, next.accountState),
		titles: sameItems(previous.titles, next.titles),
		selectedTitleIds: sameSet(previous.selectedTitleIds, next.selectedTitleIds),
		includePdfByTitleId: samePdf(previous.includePdfByTitleId, next.includePdfByTitleId),
		titleFilter: previous.titleFilter === next.titleFilter,
		showSupplementalPdfOnly: previous.showSupplementalPdfOnly === next.showSupplementalPdfOnly,
		hideUnavailableTitles: previous.hideUnavailableTitles === next.hideUnavailableTitles,
		handoffPath: previous.handoffPath === next.handoffPath,
		indexerAuthorQuery: previous.indexerAuthorQuery === next.indexerAuthorQuery,
		indexerTitleQuery: previous.indexerTitleQuery === next.indexerTitleQuery,
		releases: sameItems(previous.releases, next.releases),
		releaseFilter: previous.releaseFilter === next.releaseFilter,
		releaseSort: previous.releaseSort === next.releaseSort,
		selectedReleaseKeys: sameSet(previous.selectedReleaseKeys, next.selectedReleaseKeys),
		releaseGrabs: sameJson(previous.releaseGrabs, next.releaseGrabs),
		statusMessage: previous.statusMessage === next.statusMessage,
		// Percentage and byte counts stay on the previous job. The live fill is progress().
		activeJob: sameJson(jobShell(previous.activeJob), jobShell(next.activeJob)),
	};
	return Object.values(same).every(Boolean);
}

function jobShell(job: AcquisitionJob | null): unknown {
	if (!job) return null;
	const {
		percentage: _percentage,
		bytesDownloaded: _downloaded,
		bytesTotal: _total,
		...progress
	} = job.progress;
	return { ...job, progress };
}

function sameItems<T>(previous: readonly T[], next: readonly T[]): boolean {
	if (previous === next) return true;
	if (previous.length !== next.length) return false;
	for (let index = 0; index < previous.length; index += 1) {
		if (previous[index] !== next[index]) return false;
	}
	return true;
}

function sameSet(previous: ReadonlySet<string>, next: ReadonlySet<string>): boolean {
	if (previous === next) return true;
	if (previous.size !== next.size) return false;
	for (const value of previous) if (!next.has(value)) return false;
	return true;
}

function samePdf(previous: Record<string, boolean>, next: Record<string, boolean>): boolean {
	if (previous === next) return true;
	const keys = Object.keys(previous);
	if (keys.length !== Object.keys(next).length) return false;
	for (const key of keys) if (previous[key] !== next[key]) return false;
	return true;
}

function sameJson(previous: unknown, next: unknown): boolean {
	return previous === next || JSON.stringify(previous) === JSON.stringify(next);
}
