import { handoffMessage, statusFromAcquisitionJob, uniqueDiagnosticMessage } from './display';
import type { RemoteLibrarySnapshot, RemoteUiSnapshot } from '../../types/session';
import type { ProviderId } from '../../types/remoteSource';
import { logAppError, toUserMessage } from '../../lib/tauri/appError';
import {
	createInitialRemoteSourceState,
	snapshotRemoteSourceState,
	type RemoteSourcePatch,
	type RemoteSourceState,
	type RemoteSourceView,
} from './types';

export type RemoteSourceStateStore = {
	readonly current: () => RemoteSourceState;
	readonly snapshot: () => RemoteSourceView;
	patch(patch: RemoteSourcePatch): void;
	setAcquisitionError(cause: unknown, fallback: string, providerId: ProviderId): void;
	clearError(providerId: ProviderId): void;
	reset(): void;
};

function requestFailure(remote: RemoteUiSnapshot): string {
	for (const status of remote.lane === 'audible'
		? [remote.auth, remote.accountStatus, remote.libraryStatus]
		: [remote.accountStatus]) {
		if (status.kind === 'failed') return toUserMessage(status.error);
	}
	return '';
}

function audibleStatus(remote: RemoteUiSnapshot, library: RemoteLibrarySnapshot): string {
	const job = remote.acquisition;
	// A download outranks an earlier library-load failure; the rows it came from are still shown.
	if (job && remote.auth.kind !== 'failed' && remote.accountStatus.kind !== 'failed')
		return (job.settled ? handoffMessage(job) : null) || statusFromAcquisitionJob(job);
	const failure = requestFailure(remote);
	if (failure) return failure;
	if (remote.auth.kind === 'starting') return 'Starting Audible authorization.';
	if (remote.auth.kind === 'completing') return 'Completing Audible connection.';
	if (remote.accountStatus.kind === 'running') return 'Updating connection.';
	if (remote.libraryStatus.kind === 'running') return 'Loading Audible library.';
	if (remote.auth.kind === 'awaitingHandoff')
		return 'Complete Audible authorization in your browser, then enter the handoff path, or Connect again.';
	if (remote.libraryStatus.kind === 'succeeded') {
		return (
			uniqueDiagnosticMessage(library.diagnostics) ||
			`${library.titles.length} Audible titles loaded.`
		);
	}
	return remote.account?.message ?? '';
}

export function createRemoteSourceStateStore(
	onChange: () => void,
	choices: () => RemoteUiSnapshot,
	library: () => RemoteLibrarySnapshot,
): RemoteSourceStateStore {
	let state = createInitialRemoteSourceState();
	let errors: Partial<Record<ProviderId, string>> = {};

	const current = (): RemoteSourceState => {
		const remote = choices();
		const job = remote.acquisition;
		return {
			...state,
			providerId: remote.lane,
			providers: remote.providers,
			accountState: remote.account,
			titles: library().titles,
			selectedTitleIds: new Set(remote.selectedTitleIds),
			includePdfByTitleId: remote.includePdfByTitleId,
			releases: remote.indexer.releases,
			selectedReleaseKeys: new Set(remote.indexer.selectedReleaseKeys),
			releaseGrabs: remote.indexer.releaseGrabs,
			isGrabbing: remote.indexer.grabbing,
			isBusy:
				remote.accountStatus.kind === 'running' ||
				(remote.lane === 'audible' &&
					(remote.libraryStatus.kind === 'running' ||
						remote.auth.kind === 'completing' ||
						remote.auth.kind === 'starting')) ||
				(remote.lane === 'indexer' &&
					(remote.indexer.searching || remote.connection.save.kind === 'running')),
			activeJob: job,
			isAcquiring: remote.acquiring || (job !== null && !job.settled),
			statusByProvider: {
				audible:
					errors.audible ||
					(remote.lane === 'audible'
						? audibleStatus(remote, library())
						: job
							? (job.settled ? handoffMessage(job) : null) || statusFromAcquisitionJob(job)
							: ''),
				indexer:
					errors.indexer ||
					(remote.lane === 'indexer' ? requestFailure(remote) : '') ||
					remote.indexer.message,
			},
		};
	};
	return {
		current,
		snapshot: () => snapshotRemoteSourceState(current()),
		patch(patch) {
			state = { ...state, ...patch };
			onChange();
		},
		setAcquisitionError(cause, fallback, providerId) {
			logAppError(fallback, cause);
			errors[providerId] = toUserMessage(cause, { fallback, suppressUnknown: true });
			onChange();
		},
		clearError(providerId) {
			delete errors[providerId];
			onChange();
		},
		reset() {
			errors = {};
			state = createInitialRemoteSourceState();
			onChange();
		},
	};
}
