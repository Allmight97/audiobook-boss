import { handoffMessage, statusFromAcquisitionJob } from './display';
import type { RemoteUiSnapshot } from '../../types/session';
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
	patch(patch: RemoteSourcePatch, providerId?: ProviderId): void;
	setAcquisitionError(cause: unknown, fallback: string, providerId: ProviderId): void;
	clearError(providerId: ProviderId): void;
	reset(): void;
};

export function createRemoteSourceStateStore(
	onChange: () => void,
	choices: () => RemoteUiSnapshot,
): RemoteSourceStateStore {
	let state = createInitialRemoteSourceState();
	let errors: Partial<Record<ProviderId, string>> = {};

	function patch(patchValue: RemoteSourcePatch, providerId = state.providerId): void {
		const { statusMessage, isBusy, ...rest } = patchValue;
		if (statusMessage !== undefined) delete errors[providerId];
		state = {
			...state,
			...rest,
			...(isBusy !== undefined && providerId === state.providerId ? { isBusy } : {}),
			statusByProvider:
				statusMessage === undefined
					? state.statusByProvider
					: {
							...state.statusByProvider,
							[providerId]: statusMessage,
						},
		};
		onChange();
	}

	const current = () => {
		const remote = choices();
		const job = remote.acquisition;
		return {
			...state,
			selectedTitleIds: new Set(remote.selectedTitleIds),
			includePdfByTitleId: remote.includePdfByTitleId,
			releases: remote.indexer.releases,
			selectedReleaseKeys: new Set(remote.indexer.selectedReleaseKeys),
			releaseGrabs: remote.indexer.releaseGrabs,
			isGrabbing: remote.indexer.grabbing,
			isBusy:
				state.isBusy ||
				(state.providerId === 'indexer' &&
					(remote.indexer.searching || remote.connection.save.kind === 'running')),
			activeJob: job,
			lastJob: job,
			isAcquiring: remote.acquiring || (job !== null && !job.settled),
			statusByProvider: {
				...state.statusByProvider,
				audible:
					errors.audible ||
					(job
						? (job.settled ? handoffMessage(job) : null) || statusFromAcquisitionJob(job)
						: state.statusByProvider.audible),
				indexer: errors.indexer || remote.indexer.message || state.statusByProvider.indexer,
			},
		};
	};
	return {
		current,
		snapshot: () => snapshotRemoteSourceState(current()),
		patch,
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
