import { logAppError, toUserMessage } from '../../lib/tauri/appError';
import {
	createInitialRemoteSourceState,
	snapshotRemoteSourceState,
	type RemoteSourceState,
	type RemoteSourceView,
} from './types';

export type RemoteSourceStateStore = {
	readonly current: () => RemoteSourceState;
	readonly snapshot: () => RemoteSourceView;
	patch(patch: Partial<RemoteSourceState>): void;
	setAcquisitionError(cause: unknown, fallback: string): void;
	reset(): void;
};

export function createRemoteSourceStateStore(onChange: () => void): RemoteSourceStateStore {
	let state = createInitialRemoteSourceState();

	function patch(patchValue: Partial<RemoteSourceState>): void {
		state = {
			...state,
			...patchValue,
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
		setAcquisitionError(cause, fallback) {
			logAppError(fallback, cause);
			patch({
				statusMessage: toUserMessage(cause, { fallback, suppressUnknown: true }),
			});
		},
		reset() {
			state = createInitialRemoteSourceState();
			onChange();
		},
	};
}
