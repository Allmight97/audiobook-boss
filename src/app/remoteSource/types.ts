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

export type AcquisitionState = {
	isBusy: boolean;
	didHydrateOpenDialog: boolean;
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
	selectedReleaseGuid: string | null;
	statusMessage: string;
	activeJob: AcquisitionJobWithProgress | null;
	lastJob: AcquisitionJobWithProgress | null;
};

export type RemoteSourceState = AcquisitionState & {
	isOpen: boolean;
};

export type RemoteSourceView = RemoteSourceState;

export function providerIdFromLane(lane: AcquisitionLane): ProviderId {
	return lane;
}

export function laneFromProviderId(providerId: ProviderId): AcquisitionLane {
	return providerId === 'indexer' ? 'indexer' : 'audible';
}

export function createInitialAcquisitionState(): AcquisitionState {
	return {
		isBusy: false,
		didHydrateOpenDialog: false,
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
		selectedReleaseGuid: null,
		statusMessage: '',
		activeJob: null,
		lastJob: null,
	};
}

export function createInitialRemoteSourceState(): RemoteSourceState {
	return {
		...createInitialAcquisitionState(),
		isOpen: false,
	};
}

export function snapshotRemoteSourceState(state: RemoteSourceState): RemoteSourceView {
	return {
		...state,
		selectedTitleIds: new Set(state.selectedTitleIds),
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
		selectedReleaseGuid: null,
		statusMessage: '',
	};
}
