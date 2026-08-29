import { importIntentAtom, inputViewAtom } from '../inputSession';
import type { ImportIntent, InputView } from '../inputSession';
import { tauriClient } from '../../lib/tauri/client';
import { Effect } from '../../lib/effect/appEffect';
import { remoteSourceProviderId, type RemoteInputHandoffResult } from './types';
import { ORDER_LOCKED_IMPORT_MESSAGE } from './workflow';
import type { RemoteSourceWorkflowServices } from './workflow';

export type RemoteSourceServiceGet = {
	(atom: typeof inputViewAtom): InputView;
	readonly setResult: (
		atom: typeof importIntentAtom,
		value: ImportIntent,
	) => Effect.Effect<unknown, unknown>;
};

export function makeProductionRemoteSourceServices(
	get: RemoteSourceServiceGet,
): RemoteSourceWorkflowServices {
	return {
		getAccountState: () => tauriClient.getRemoteSourceAccountState(remoteSourceProviderId),
		startAuth: () => tauriClient.startRemoteSourceAuth(remoteSourceProviderId),
		openAuthorizationUrl: (url) => tauriClient.openUrl(url),
		completeAuth: (responseUrlHandoffPath) =>
			tauriClient.completeRemoteSourceAuth({
				providerId: remoteSourceProviderId,
				responseUrlHandoffPath,
			}),
		logout: () => tauriClient.logoutRemoteSourceAccount(remoteSourceProviderId),
		loadLibrary: () => tauriClient.loadRemoteSourceLibrary(remoteSourceProviderId),
		startAcquisition: (selections) =>
			tauriClient.startRemoteSourceAcquisition({
				providerId: remoteSourceProviderId,
				selections: [...selections],
			}),
		getAcquisitionStatus: (jobId) => tauriClient.getRemoteSourceAcquisitionStatus(jobId),
		cancelAcquisition: (jobId) => tauriClient.cancelRemoteSourceAcquisition(jobId),
		purgeSession: (jobId) => tauriClient.purgeRemoteSourceSession(jobId),
		importMaterializedPaths: (paths) => importMaterializedPathsThroughInput(get, paths),
		sleep: (ms) => new Promise((resolve) => window.setTimeout(resolve, ms)),
	};
}

async function importMaterializedPathsThroughInput(
	get: RemoteSourceServiceGet,
	paths: readonly string[],
): Promise<RemoteInputHandoffResult> {
	const view = get(inputViewAtom);
	if (view.orderLocked) {
		return { status: 'blocked', message: ORDER_LOCKED_IMPORT_MESSAGE };
	}

	try {
		await Effect.runPromise(
			get.setResult(importIntentAtom, { type: 'importPaths', paths: [...paths] }),
		);
		const after = get(inputViewAtom);
		if (
			after.errorMessage &&
			!paths.some((path) => after.files.some((file) => file.path === path))
		) {
			return { status: 'failed', message: after.errorMessage };
		}
		return {
			status: 'imported',
			fileList: {
				files: [...after.files],
				selectedDecoders: after.files.map(() => null),
				totalDuration: 0,
				totalSize: 0,
				validCount: after.files.filter((file) => file.isValid).length,
				invalidCount: after.files.filter((file) => !file.isValid).length,
			},
		};
	} catch (cause) {
		return {
			status: 'failed',
			message: cause instanceof Error ? cause.message : 'Failed to import acquired titles.',
		};
	}
}
