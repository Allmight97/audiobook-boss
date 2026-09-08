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
	reset(): void;
};

export function createRemoteSourceStateStore(onChange: () => void): RemoteSourceStateStore {
	let state = createInitialRemoteSourceState();

	function patch(patchValue: RemoteSourcePatch, providerId = state.providerId): void {
		const { statusMessage, isBusy, ...rest } = patchValue;
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
			selectedTitleIds: patchValue.selectedTitleIds
				? new Set(patchValue.selectedTitleIds)
				: state.selectedTitleIds,
		};
		onChange();
	}

	return {
		current: () => state,
		snapshot: () => snapshotRemoteSourceState(state),
		patch,
		setAcquisitionError(cause, fallback, providerId) {
			logAppError(fallback, cause);
			patch(
				{
					statusMessage: toUserMessage(cause, { fallback, suppressUnknown: true }),
				},
				providerId,
			);
		},
		reset() {
			state = createInitialRemoteSourceState();
			onChange();
		},
	};
}
