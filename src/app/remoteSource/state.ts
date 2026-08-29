import { logAppError, toUserMessage } from '../../lib/tauri/appError';
import {
	createInitialRemoteSourceState,
	snapshotRemoteSourceState,
	type RemoteSourceState,
	type RemoteSourceView,
} from './types';

export let remoteSourceState: RemoteSourceState = createInitialRemoteSourceState();

export function snapshotRemoteSourceView(): RemoteSourceView {
	return snapshotRemoteSourceState(remoteSourceState);
}

export function resetRemoteSourceState(): void {
	remoteSourceState = createInitialRemoteSourceState();
}

export function setAcquisitionError(cause: unknown, fallback: string): void {
	logAppError(fallback, cause);
	remoteSourceState = {
		...remoteSourceState,
		statusMessage: toUserMessage(cause, { fallback, suppressUnknown: true }),
	};
}

export function patchRemoteSourceState(patch: Partial<RemoteSourceState>): void {
	remoteSourceState = {
		...remoteSourceState,
		...patch,
		selectedTitleIds: patch.selectedTitleIds
			? new Set(patch.selectedTitleIds)
			: remoteSourceState.selectedTitleIds,
	};
}
