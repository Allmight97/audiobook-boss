import type { RemoteUiSnapshot } from '../../types/session';
import type {
	ProviderId,
	RemoteRelease,
	RemoteSourceAccountState,
	RemoteSourceProviderCapabilities,
} from '../../types/remoteSource';

type AcquisitionState = {
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
	releaseGrabs: Readonly<RemoteUiSnapshot['indexer']['releaseGrabs']>;
	canGrabSelection: boolean;
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
	| 'releaseGrabs'
	| 'canGrabSelection';
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
