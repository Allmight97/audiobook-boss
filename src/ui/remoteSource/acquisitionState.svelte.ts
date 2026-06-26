import type { ProviderId, RemoteSourceAccountState, RemoteTitle } from '../../types/remoteSource';
import { logAppError, toUserMessage } from '../../lib/tauri/appError';
import type { AcquisitionJobWithProgress } from './remoteSourceAcquireDialogHelpers';

export const remoteSourceProviderId: ProviderId = 'audible';

/**
 * Provider-neutral acquisition dialog state shape.
 * The owning Svelte component creates this via $state() so each
 * component instance / test render gets isolated reactive state.
 */
export interface AcquisitionState {
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
}

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

export function setAcquisitionError(s: AcquisitionState, cause: unknown, fallback: string): void {
	logAppError(fallback, cause);
	s.statusMessage = toUserMessage(cause, { fallback, suppressUnknown: true });
}
