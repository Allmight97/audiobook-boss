import type {
	ProviderId,
	RemoteAcquisitionSnapshot,
	RemoteUiSnapshot,
} from '../../lib/generated/tauri';
import { logFrontendFailure } from '../../lib/frontendLogBridge';
import { toUserMessage } from '../../lib/tauri/appError';
import {
	createInitialRemoteSourceState,
	type RemoteSourcePatch,
	type RemoteSourceView,
} from './types';

export type RemoteSourceStateStore = {
	readonly current: () => RemoteSourceView;
	readonly snapshot: () => RemoteSourceView;
	patch: (patch: RemoteSourcePatch) => void;
	setAcquisitionError: (cause: unknown, fallback: string, providerId: ProviderId) => void;
	clearError: (providerId: ProviderId) => void;
	reset: () => void;
};

function laneMessage(remote: RemoteUiSnapshot, job: RemoteAcquisitionSnapshot['job']): string {
	if (remote.lane === 'indexer') return remote.indexer.message;
	return job && job.state.kind !== 'done' ? job.progress.message : remote.audibleMessage;
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
				readonly acquisition: RemoteAcquisitionSnapshot;
				readonly localVersion: number;
				readonly view: RemoteSourceView;
		  }
		| undefined;
	const changed = () => {
		localVersion += 1;
		onChange();
	};

	const current = (): RemoteSourceView => {
		const remote = choices();
		const { job } = acquisition();
		return {
			...state,
			providerId: remote.lane,
			providers: remote.providers,
			accountState: remote.account,
			selectedTitleIds: selectedTitleIds(remote.selectedTitleIds),
			includePdfByTitleId: remote.includePdfByTitleId,
			releases: remote.indexer.releases,
			selectedReleaseKeys: selectedReleaseKeys(remote.indexer.selectedReleaseKeys),
			releaseGrabs: remote.indexer.releaseGrabs,
			canGrabSelection: remote.indexer.canGrabSelection,
			isBusy: remote.busy,
			statusMessage: errors[remote.lane] || laneMessage(remote, job),
		};
	};
	return {
		current,
		snapshot: () => {
			const remote = choices();
			const download = acquisition();
			if (
				cached?.remote !== remote ||
				cached.acquisition !== download ||
				cached.localVersion !== localVersion
			) {
				cached = {
					remote,
					acquisition: download,
					localVersion,
					view: current(),
				};
			}
			return cached.view;
		},
		patch(patch) {
			state = { ...state, ...patch };
			changed();
		},
		setAcquisitionError(cause, fallback, providerId) {
			logFrontendFailure(fallback, cause);
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
