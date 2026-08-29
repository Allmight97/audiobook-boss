import type { FileListInfo } from '../../types/audio';
import type { ProviderId, RemoteSourceAccountState, RemoteTitle } from '../../types/remoteSource';
import type { AcquisitionJobWithProgress } from './display';

export const remoteSourceProviderId: ProviderId = 'audible';

export type RemoteInputHandoffResult =
	| { readonly status: 'imported'; readonly fileList: FileListInfo | null }
	| { readonly status: 'blocked'; readonly message: string }
	| { readonly status: 'failed'; readonly message: string };

export type AcquisitionState = {
	isBusy: boolean;
	didHydrateOpenDialog: boolean;
	accountState: RemoteSourceAccountState | null;
	titles: RemoteTitle[];
	selectedTitleIds: Set<string>;
	includePdfByTitleId: Record<string, boolean>;
	titleFilter: string;
	showSupplementalPdfOnly: boolean;
	hideUnavailableTitles: boolean;
	handoffPath: string;
	statusMessage: string;
	activeJob: AcquisitionJobWithProgress | null;
	lastJob: AcquisitionJobWithProgress | null;
};

export type RemoteSourceState = AcquisitionState & {
	isOpen: boolean;
};

export type RemoteSourceView = RemoteSourceState;

export function createInitialAcquisitionState(): AcquisitionState {
	return {
		isBusy: false,
		didHydrateOpenDialog: false,
		accountState: null,
		titles: [],
		selectedTitleIds: new Set(),
		includePdfByTitleId: {},
		titleFilter: '',
		showSupplementalPdfOnly: false,
		hideUnavailableTitles: false,
		handoffPath: '',
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
	};
}
