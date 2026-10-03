import type {
	AcquisitionJob,
	ProviderId,
	RemoteRelease,
	RemoteSourceAccountState,
	RemoteSourceProviderCapabilities,
	RemoteTitle,
} from '../../types/remoteSource';

export type ReleaseGrabState = {
	status: 'queued' | 'sending' | 'sent' | 'error';
	message: string;
};

export type AcquisitionState = {
	isBusy: boolean;
	providerId: ProviderId;
	providers: RemoteSourceProviderCapabilities[];
	accountState: RemoteSourceAccountState | null;
	titles: RemoteTitle[];
	selectedTitleIds: Set<string>;
	includePdfByTitleId: Record<string, boolean>;
	titleFilter: string;
	showSupplementalPdfOnly: boolean;
	hideUnavailableTitles: boolean;
	handoffPath: string;
	indexerAuthorQuery: string;
	indexerTitleQuery: string;
	releases: RemoteRelease[];
	releaseFilter: string;
	releaseSort: 'seeders' | 'size';
	selectedReleaseKeys: Set<string>;
	releaseGrabs: Record<string, ReleaseGrabState>;
	statusMessage: string;
	activeJob: AcquisitionJob | null;
};

export type RemoteSourceState = Omit<AcquisitionState, 'statusMessage'> & {
	isOpen: boolean;
	isGrabbing: boolean;
	isAcquiring: boolean;
	statusByProvider: Record<ProviderId, string>;
};

export type RemoteSourceView = Omit<
	RemoteSourceState,
	'isGrabbing' | 'isAcquiring' | 'statusByProvider'
> & {
	statusMessage: string;
};

type EngineFields =
	| 'selectedTitleIds'
	| 'includePdfByTitleId'
	| 'releases'
	| 'selectedReleaseKeys'
	| 'releaseGrabs'
	| 'isGrabbing'
	| 'isAcquiring'
	| 'activeJob';
export type RemoteSourceLocalState = Omit<RemoteSourceState, EngineFields>;
export type RemoteSourcePatch = Partial<RemoteSourceLocalState> & { statusMessage?: string };
export function createInitialRemoteSourceState(): RemoteSourceLocalState {
	return {
		isBusy: false,
		providerId: 'audible',
		providers: [],
		accountState: null,
		titles: [],
		titleFilter: '',
		showSupplementalPdfOnly: false,
		hideUnavailableTitles: false,
		handoffPath: '',
		indexerAuthorQuery: '',
		indexerTitleQuery: '',
		releaseFilter: '',
		releaseSort: 'seeders',
		isOpen: false,
		statusByProvider: { audible: '', indexer: '' },
	};
}

export function snapshotRemoteSourceState(state: RemoteSourceState): RemoteSourceView {
	const { isGrabbing, isAcquiring, statusByProvider, ...view } = state;
	return {
		...view,
		statusMessage: statusByProvider[state.providerId],
		isBusy: state.isBusy || isGrabbing || (state.providerId === 'audible' && isAcquiring),
		selectedTitleIds: new Set(state.selectedTitleIds),
		selectedReleaseKeys: new Set(state.selectedReleaseKeys),
		releaseGrabs: { ...state.releaseGrabs },
		includePdfByTitleId: { ...state.includePdfByTitleId },
		titles: [...state.titles],
		providers: [...state.providers],
		releases: [...state.releases],
	};
}

export function laneSelectionResetPatch(): Partial<AcquisitionState> {
	return {
		titleFilter: '',
		showSupplementalPdfOnly: false,
		hideUnavailableTitles: false,
		indexerAuthorQuery: '',
		indexerTitleQuery: '',
		releaseFilter: '',
		releaseSort: 'seeders',
	};
}
