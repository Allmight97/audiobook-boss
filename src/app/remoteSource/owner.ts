import { createMemo, createSignal, onCleanup, type Accessor } from 'solid-js';
import type { AcquisitionLane } from '../../types/appSettings';
import { tauriClient } from '../../lib/tauri/client';
import type { RemoteUiIntent } from '../../types/session';
import type { AcquisitionJob, RemoteRelease, RemoteTitle } from '../../types/remoteSource';
import {
	createIndexerConnectionSettings,
	type IndexerConnectionSettingsView,
} from './indexerConnection';
import type { EngineLink } from '../engineLink';
import { createRemoteSourceStateStore } from './state';
import {
	selectedRemoteTitleSummaryText,
	visibleRemoteReleases,
	visibleRemoteTitles,
} from './selection';
import type { RemoteSourceView } from './types';

type RemoteSourceAction =
	| { readonly type: 'startAuth' | 'completeAuth' | 'logout' | 'loadLibrary' }
	| { readonly type: 'searchReleases' }
	| { readonly type: 'grabSelectedReleases' }
	| { readonly type: 'grabRelease'; readonly key: string }
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
			return { kind: 'disconnect' };
		case 'loadLibrary':
			return { kind: 'refreshLibrary' };
	}
}

function actionIntent(
	action: RemoteSourceAction,
	view: RemoteSourceView,
	job: AcquisitionJob | null,
): RemoteUiIntent | null {
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
			return { kind: 'grabRelease', key: action.key };
		case 'acquireSelected':
			return { kind: 'acquireSelected' };
		case 'cancelActiveAcquisition':
			return job ? { kind: 'cancelAcquisition', jobId: job.jobId } : null;
		default:
			return accountIntent(action.type, view.handoffPath);
	}
}

export type RemoteSourceOwner = {
	readonly view: Accessor<RemoteSourceView>;
	readonly isBusy: Accessor<boolean>;
	/** Library titles left by the filter, PDF, and availability choices. */
	readonly visibleTitles: Accessor<readonly RemoteTitle[]>;
	readonly selectedTitleSummary: Accessor<string>;
	isTitleSelected: (titleId: string) => boolean;
	includesPdf: (titleId: string) => boolean;
	readonly visibleReleases: Accessor<readonly RemoteRelease[]>;
	isReleaseSelected: (key: string) => boolean;
	releaseGrab: (key: string) => RemoteSourceView['releaseGrabs'][string] | undefined;
	readonly hiddenReleaseSelectionCount: Accessor<number>;
	readonly acquisition: Accessor<AcquisitionJob | null>;
	readonly acquisitionTitle: Accessor<RemoteTitle | undefined>;
	readonly indexerConnection: Accessor<IndexerConnectionSettingsView>;
	open: (options: { readonly lane: AcquisitionLane }) => Promise<void>;
	selectLane: (lane: AcquisitionLane) => Promise<void>;
	close: () => void;
	editSearch: (
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
	) => void;
	toggleTitle: (titleId: string) => void;
	clearTitleSelection: () => void;
	toggleSupplementalPdf: (titleId: string) => void;
	selectRelease: (key: string, options?: { multi: boolean }) => void;
	runAction: (action: RemoteSourceAction) => Promise<void>;
	loadIndexerConnectionSettings: () => Promise<void>;
	patchIndexerConnectionSettings: (
		patch: Partial<
			Pick<IndexerConnectionSettingsView, 'baseUrlDraft' | 'apiKeyDraft' | 'categoryIdsDraft'>
		>,
	) => void;
	saveIndexerConnectionSettings: () => Promise<void>;
	testIndexerConnection: () => Promise<void>;
	/** Forgets what was typed in the Indexer form, so it shows the engine's draft. */
	discardIndexerConnectionEdits: () => void;
	reset: () => void;
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
		() => deps.link.remoteAcquisition(),
	);
	const indexerConnection = createIndexerConnectionSettings(deps.link);

	const openAuthorizationUrl = deps.openAuthorizationUrl ?? tauriClient.openUrl;
	let generation = 0;
	let disposed = false;
	onCleanup(() => {
		disposed = true;
		generation += 1;
	});

	const view = () => {
		viewRev();
		return state.snapshot();
	};
	const titles = () => deps.link.remoteLibrary().titles;
	const selectedTitleIds = createMemo(() => view().selectedTitleIds);
	const includePdfByTitleId = createMemo(() => view().includePdfByTitleId);
	const titleFilter = createMemo(() => view().titleFilter);
	const showSupplementalPdfOnly = createMemo(() => view().showSupplementalPdfOnly);
	const hideUnavailableTitles = createMemo(() => view().hideUnavailableTitles);
	const visibleTitles = createMemo(() =>
		visibleRemoteTitles(titles(), {
			titleFilter: titleFilter(),
			showSupplementalPdfOnly: showSupplementalPdfOnly(),
			hideUnavailableTitles: hideUnavailableTitles(),
		}),
	);
	const titleById = createMemo(() => new Map(titles().map((title) => [title.titleId, title])));
	const releases = createMemo(() => view().releases);
	const selectedReleaseKeys = createMemo(() => view().selectedReleaseKeys);
	const releaseGrabs = createMemo(() => view().releaseGrabs);
	const releaseFilter = createMemo(() => view().releaseFilter);
	const releaseSort = createMemo(() => view().releaseSort);
	const visibleReleases = createMemo(() =>
		visibleRemoteReleases(releases(), {
			releaseFilter: releaseFilter(),
			releaseSort: releaseSort(),
		}),
	);
	const isReleaseSelected = (key: string) => selectedReleaseKeys().has(key);
	const selectedReleases = createMemo(() =>
		releases().filter((release) => isReleaseSelected(release.key)),
	);
	const acquisition = () => deps.link.remoteAcquisition().job;

	return {
		view,
		isBusy: createMemo(() => view().isBusy),
		visibleTitles,
		selectedTitleSummary: createMemo(() =>
			selectedRemoteTitleSummaryText(selectedTitleIds(), visibleTitles()),
		),
		isTitleSelected: (titleId) => selectedTitleIds().has(titleId),
		includesPdf: (titleId) => includePdfByTitleId()[titleId] === true,
		visibleReleases,
		isReleaseSelected,
		releaseGrab: (key) => releaseGrabs()[key],
		hiddenReleaseSelectionCount: createMemo(
			() =>
				selectedReleases().length -
				visibleReleases().filter((release) => isReleaseSelected(release.key)).length,
		),
		acquisition,
		acquisitionTitle: createMemo(() => {
			const titleId = acquisition()?.progress.currentTitleId;
			return titleId ? titleById().get(titleId) : undefined;
		}),
		indexerConnection: indexerConnection.view,
		async open({ lane }) {
			if (disposed) return;
			const started = generation;
			state.patch({ isOpen: true });
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
		selectRelease(key, options) {
			deps.link.post({
				kind: 'remote',
				intent: { kind: 'selectRelease', key, multi: options?.multi ?? false },
			});
		},
		async runAction(action) {
			const providerId = state.current().providerId;
			const started = generation;
			if (disposed) return;
			state.clearError(providerId);
			try {
				const intent = actionIntent(action, state.current(), deps.link.remoteAcquisition().job);
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
			await indexerConnection.save();
		},
		testIndexerConnection() {
			return indexerConnection.testConnection();
		},
		discardIndexerConnectionEdits() {
			indexerConnection.reset();
		},
		reset() {
			generation += 1;
			indexerConnection.reset();
			state.reset();
		},
	};
}
