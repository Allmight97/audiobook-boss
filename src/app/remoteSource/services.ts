import type { InputView } from '../inputSession';
import { tauriClient } from '../../lib/tauri/client';
import type {
	ProviderId,
	RemoteReleaseGrabRequest,
	RemoteReleaseSearchRequest,
} from '../../types/remoteSource';
import type { RemoteInputHandoffResult } from './types';
import { ORDER_LOCKED_IMPORT_MESSAGE } from './workflow';
import type { RemoteSourceWorkflowServices } from './workflow';

export type RemoteSourceInputBridge = {
	readonly inputView: () => InputView;
	readonly importPaths: (paths: readonly string[]) => Promise<void>;
};

export function makeProductionRemoteSourceServices(
	bridge: RemoteSourceInputBridge,
): RemoteSourceWorkflowServices {
	return {
		listProviders: () => tauriClient.listRemoteSourceProviders(),
		getAccountState: (providerId: ProviderId) =>
			tauriClient.getRemoteSourceAccountState(providerId),
		startAuth: (providerId: ProviderId) => tauriClient.startRemoteSourceAuth(providerId),
		openAuthorizationUrl: (url) => tauriClient.openUrl(url),
		completeAuth: (providerId, responseUrlHandoffPath) =>
			tauriClient.completeRemoteSourceAuth({
				providerId,
				responseUrlHandoffPath,
			}),
		logout: (providerId: ProviderId) => tauriClient.logoutRemoteSourceAccount(providerId),
		loadLibrary: (providerId: ProviderId) => tauriClient.loadRemoteSourceLibrary(providerId),
		searchReleases: (request: RemoteReleaseSearchRequest) =>
			tauriClient.searchRemoteSourceReleases(request),
		grabRelease: (request: RemoteReleaseGrabRequest) =>
			tauriClient.grabRemoteSourceRelease(request),
		startAcquisition: (providerId, selections) =>
			tauriClient.startRemoteSourceAcquisition({
				providerId,
				selections: [...selections],
			}),
		getAcquisitionStatus: (jobId) => tauriClient.getRemoteSourceAcquisitionStatus(jobId),
		cancelAcquisition: (jobId) => tauriClient.cancelRemoteSourceAcquisition(jobId),
		purgeSession: (jobId) => tauriClient.purgeRemoteSourceSession(jobId),
		importMaterializedPaths: (paths) => importMaterializedPathsThroughInput(bridge, paths),
		sleep: (ms) => new Promise((resolve) => window.setTimeout(resolve, ms)),
	};
}

export function makeProductionIndexerConnectionServices() {
	return {
		getIndexerConnection: () => tauriClient.getRemoteSourceIndexerConnection(),
		updateIndexerConnection: (
			update: Parameters<typeof tauriClient.updateRemoteSourceIndexerConnection>[0],
		) => tauriClient.updateRemoteSourceIndexerConnection(update),
		testIndexerConnection: () => tauriClient.testRemoteSourceIndexerConnection(),
	};
}

async function importMaterializedPathsThroughInput(
	bridge: RemoteSourceInputBridge,
	paths: readonly string[],
): Promise<RemoteInputHandoffResult> {
	const view = bridge.inputView();
	if (view.orderLocked) {
		return { status: 'blocked', message: ORDER_LOCKED_IMPORT_MESSAGE };
	}

	try {
		await bridge.importPaths(paths);
		const after = bridge.inputView();
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
