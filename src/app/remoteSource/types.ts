import type {
	ProviderId,
	RemoteRelease,
	RemoteSourceAccountState,
	RemoteSourceProviderCapabilities,
} from '../../types/remoteSource';

export type ReleaseGrabState = {
	status: 'queued' | 'sending' | 'sent' | 'error';
	message: string;
};

export type AcquisitionState = {
	isBusy: boolean;
	providerId: ProviderId;
	providers: readonly RemoteSourceProviderCapabilities[];
	accountState: RemoteSourceAccountState | null;
	selectedTitleIds: ReadonlySet<string>;
	includePdfByTitleId: Readonly<Record<string, boolean>>;
	titleFilter: string;
	showSupplementalPdfOnly: boolean;
	hideUnavailableTitles: boolean;
	handoffPath: string;
	indexerAuthorQuery: string;
	indexerTitleQuery: string;
	releases: readonly RemoteRelease[];
	releaseFilter: string;
	releaseSort: 'seeders' | 'size';
	selectedReleaseKeys: ReadonlySet<string>;
	releaseGrabs: Readonly<Record<string, ReleaseGrabState>>;
	statusMessage: string;
};

export type RemoteSourceState = Omit<AcquisitionState, 'statusMessage'> & {
	isOpen: boolean;
	statusByProvider: Record<ProviderId, string>;
};

export type RemoteSourceView = Omit<RemoteSourceState, 'statusByProvider'> & {
	statusMessage: string;
};

type EngineFields =
	| 'isBusy'
	| 'providerId'
	| 'providers'
	| 'accountState'
	| 'statusByProvider'
	| 'selectedTitleIds'
	| 'includePdfByTitleId'
	| 'releases'
	| 'selectedReleaseKeys'
	| 'releaseGrabs';
export type RemoteSourceLocalState = Omit<RemoteSourceState, EngineFields>;
export type RemoteSourcePatch = Partial<RemoteSourceLocalState>;
export function createInitialRemoteSourceState(): RemoteSourceLocalState {
	return {
		titleFilter: '',
		showSupplementalPdfOnly: false,
		hideUnavailableTitles: false,
		handoffPath: '',
		indexerAuthorQuery: '',
		indexerTitleQuery: '',
		releaseFilter: '',
		releaseSort: 'seeders',
		isOpen: false,
	};
}

export function snapshotRemoteSourceState(state: RemoteSourceState): RemoteSourceView {
	const { statusByProvider, ...view } = state;
	return { ...view, statusMessage: statusByProvider[state.providerId] };
}
