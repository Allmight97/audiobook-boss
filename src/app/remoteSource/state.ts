import { handoffMessage, statusFromAcquisitionJob, uniqueDiagnosticMessage } from './display';
import type {
	RemoteAcquisitionSnapshot,
	RemoteLibrarySnapshot,
	RemoteUiSnapshot,
} from '../../types/session';
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

function connectedAccountNote(remote: RemoteUiSnapshot): string {
	if (remote.account?.status !== 'connected') return '';
	return remote.account.message ?? '';
}

/** A connected account message is how the engine says a sign-in will not last. */
function withConnectedAccountNote(status: string, remote: RemoteUiSnapshot): string {
	const note = connectedAccountNote(remote);
	if (!note || status.includes(note)) return status;
	return status ? `${status} ${note}` : note;
}

function audibleStatus(
	remote: RemoteUiSnapshot,
	library: RemoteLibrarySnapshot,
	job: RemoteAcquisitionSnapshot['job'],
): string {
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
		const loaded =
			uniqueDiagnosticMessage(library.diagnostics) ||
			`${library.titles.length} Audible titles loaded.`;
		return withConnectedAccountNote(loaded, remote);
	}
	return remote.account?.message ?? '';
}

function indexerStatus(remote: RemoteUiSnapshot): string {
	if (remote.lane !== 'indexer') return remote.indexer.message;
	const status = requestFailure(remote) || remote.indexer.message;
	return withConnectedAccountNote(status, remote);
}

function setOf(): (ids: readonly string[]) => Set<string> {
	let source: readonly string[] | undefined;
	let set = new Set<string>();
	return (ids) => {
		if (ids !== source) {
			source = ids;
			set = new Set(ids);
		}
		return set;
	};
}

export function createRemoteSourceStateStore(
	onChange: () => void,
	choices: () => RemoteUiSnapshot,
	library: () => RemoteLibrarySnapshot,
	acquisition: () => RemoteAcquisitionSnapshot,
): RemoteSourceStateStore {
	let state = createInitialRemoteSourceState();
	let errors: Partial<Record<ProviderId, string>> = {};
	const selectedTitleIds = setOf();
	const selectedReleaseKeys = setOf();
	let localVersion = 0;
	let cached:
		| {
				readonly remote: RemoteUiSnapshot;
				readonly library: RemoteLibrarySnapshot;
				readonly acquisition: RemoteAcquisitionSnapshot;
				readonly localVersion: number;
				readonly view: RemoteSourceView;
		  }
		| undefined;
	const changed = () => {
		localVersion += 1;
		onChange();
	};

	const current = (): RemoteSourceState => {
		const remote = choices();
		const { acquiring, job } = acquisition();
		return {
			...state,
			providerId: remote.lane,
			providers: remote.providers,
			accountState: remote.account,
			titles: library().titles,
			selectedTitleIds: selectedTitleIds(remote.selectedTitleIds),
			includePdfByTitleId: remote.includePdfByTitleId,
			releases: remote.indexer.releases,
			selectedReleaseKeys: selectedReleaseKeys(remote.indexer.selectedReleaseKeys),
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
			isAcquiring: acquiring || (job !== null && !job.settled),
			statusByProvider: {
				audible:
					errors.audible ||
					(remote.lane === 'audible'
						? audibleStatus(remote, library(), job)
						: job
							? (job.settled ? handoffMessage(job) : null) || statusFromAcquisitionJob(job)
							: ''),
				indexer: errors.indexer || indexerStatus(remote),
			},
		};
	};
	return {
		current,
		snapshot: () => {
			const remote = choices();
			const titles = library();
			const download = acquisition();
			if (
				cached?.remote !== remote ||
				cached.library !== titles ||
				cached.acquisition !== download ||
				cached.localVersion !== localVersion
			) {
				cached = {
					remote,
					library: titles,
					acquisition: download,
					localVersion,
					view: snapshotRemoteSourceState(current()),
				};
			}
			return cached.view;
		},
		patch(patch) {
			state = { ...state, ...patch };
			changed();
		},
		setAcquisitionError(cause, fallback, providerId) {
			logAppError(fallback, cause);
			errors[providerId] = toUserMessage(cause, { fallback, suppressUnknown: true });
			changed();
		},
		clearError(providerId) {
			delete errors[providerId];
			changed();
		},
		reset() {
			errors = {};
			state = createInitialRemoteSourceState();
			changed();
		},
	};
}
