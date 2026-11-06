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
	includePdfByTitleId: Readonly<Partial<Record<string, boolean>>>;
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

export type RemoteSourceView = AcquisitionState & { isOpen: boolean };

type EngineFields =
	| 'isBusy'
	| 'providerId'
	| 'providers'
	| 'accountState'
	| 'statusMessage'
	| 'selectedTitleIds'
	| 'includePdfByTitleId'
	| 'releases'
	| 'selectedReleaseKeys'
	| 'releaseGrabs';
export type RemoteSourceLocalState = Omit<RemoteSourceView, EngineFields>;
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
