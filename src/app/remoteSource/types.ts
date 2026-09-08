import type { AcquisitionLane } from '../../types/appSettings';
import type { FileListInfo } from '../../types/audio';
import type {
	ProviderId,
	RemoteRelease,
	RemoteSourceAccountState,
	RemoteSourceProviderCapabilities,
	RemoteTitle,
} from '../../types/remoteSource';
import type { AcquisitionJobWithProgress } from './display';

export type RemoteInputHandoffResult =
	| { readonly status: 'imported'; readonly fileList: FileListInfo | null }
	| { readonly status: 'blocked'; readonly message: string }
	| { readonly status: 'failed'; readonly message: string };

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
	activeJob: AcquisitionJobWithProgress | null;
	lastJob: AcquisitionJobWithProgress | null;
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

export type RemoteSourcePatch = Partial<RemoteSourceState> & { statusMessage?: string };

export function providerIdFromLane(lane: AcquisitionLane): ProviderId {
	return lane;
}

export function createInitialAcquisitionState(): AcquisitionState {
	return {
		isBusy: false,
		providerId: 'audible',
		providers: [],
		accountState: null,
		titles: [],
		selectedTitleIds: new Set(),
		includePdfByTitleId: {},
		titleFilter: '',
		showSupplementalPdfOnly: false,
		hideUnavailableTitles: false,
		handoffPath: '',
		indexerAuthorQuery: '',
		indexerTitleQuery: '',
		releases: [],
		releaseFilter: '',
		releaseSort: 'seeders',
		selectedReleaseKeys: new Set(),
		releaseGrabs: {},
		statusMessage: '',
		activeJob: null,
		lastJob: null,
	};
}

export function createInitialRemoteSourceState(): RemoteSourceState {
	const { statusMessage, ...initial } = createInitialAcquisitionState();
	return {
		...initial,
		isOpen: false,
		isGrabbing: false,
		isAcquiring: false,
		statusByProvider: { audible: statusMessage, indexer: '' },
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
		selectedTitleIds: new Set(),
		includePdfByTitleId: {},
		titleFilter: '',
		showSupplementalPdfOnly: false,
		hideUnavailableTitles: false,
		indexerAuthorQuery: '',
		indexerTitleQuery: '',
		releases: [],
		releaseFilter: '',
		releaseSort: 'seeders',
		selectedReleaseKeys: new Set(),
		releaseGrabs: {},
	};
}
