import { normalizeAppError } from '../../lib/tauri/appError';
import { tauriClient } from '../../lib/tauri/client';
import type { ProviderId } from '../../types/remoteSource';
import { isTitleAcquirable, uniqueDiagnosticMessage } from './remoteSourceAcquireDialogHelpers';
import type { AcquisitionState } from './acquisitionState.svelte';

const providerId: ProviderId = 'audible';

function setError(s: AcquisitionState, cause: unknown, fallback: string): void {
	const error = normalizeAppError(cause, fallback);
	console.error(`${fallback} code=${error.code} category=${error.category}`);
	s.statusMessage = error.code === 'unknown_error' ? fallback : error.message;
}

async function refreshAccountState(s: AcquisitionState): Promise<void> {
	s.accountState = await tauriClient.getRemoteSourceAccountState(providerId);
}

async function loadLibrary(s: AcquisitionState): Promise<void> {
	s.isBusy = true;
	try {
		const library = await tauriClient.loadRemoteSourceLibrary(providerId);
		s.titles = library.titles;
		const selectableTitleIds = new Set(
			library.titles.filter((title) => isTitleAcquirable(title)).map((title) => title.titleId),
		);
		s.selectedTitleIds = new Set(
			[...s.selectedTitleIds].filter((titleId) => selectableTitleIds.has(titleId)),
		);
		s.includePdfByTitleId = Object.fromEntries(
			library.titles.map((title) => [title.titleId, title.supplementalPdfAvailable]),
		);
		s.statusMessage =
			library.diagnostics.length > 0
				? uniqueDiagnosticMessage(library.diagnostics)
				: `${library.titles.length} Audible titles loaded.`;
	} catch (cause) {
		setError(s, cause, 'Failed to load Audible library.');
	} finally {
		s.isBusy = false;
	}
}

// -- account lifecycle controller factory --

export function createAccountController(s: AcquisitionState) {
	return {
		async hydrateOpenDialog(): Promise<void> {
			s.isBusy = true;
			try {
				await refreshAccountState(s);
				if (s.accountState?.status === 'connected') {
					await loadLibrary(s);
				}
			} catch (cause) {
				setError(s, cause, 'Failed to load remote source state.');
			} finally {
				s.isBusy = false;
			}
		},

		async startAuth(): Promise<void> {
			s.isBusy = true;
			try {
				const response = await tauriClient.startRemoteSourceAuth(providerId);
				s.statusMessage = response.message;
				await tauriClient.openUrl(response.authorizationUrl);
			} catch (cause) {
				setError(s, cause, 'Failed to start Audible auth.');
			} finally {
				s.isBusy = false;
			}
		},

		async completeAuth(): Promise<void> {
			s.isBusy = true;
			try {
				s.accountState = await tauriClient.completeRemoteSourceAuth({
					providerId,
					responseUrlHandoffPath: s.handoffPath.trim() || undefined,
				});
				s.statusMessage = 'Audible connected.';
				await loadLibrary(s);
			} catch (cause) {
				setError(s, cause, 'Failed to complete Audible auth.');
			} finally {
				s.isBusy = false;
			}
		},

		async logout(): Promise<void> {
			s.isBusy = true;
			try {
				s.accountState = await tauriClient.logoutRemoteSourceAccount(providerId);
				s.titles = [];
				s.selectedTitleIds = new Set();
				s.includePdfByTitleId = {};
				s.activeJob = null;
				s.lastJob = null;
				s.statusMessage = 'Audible disconnected.';
			} catch (cause) {
				setError(s, cause, 'Failed to disconnect Audible.');
			} finally {
				s.isBusy = false;
			}
		},

		async loadLibrary(): Promise<void> {
			await loadLibrary(s);
		},
	};
}
